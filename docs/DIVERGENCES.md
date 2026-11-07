# DIVERGENCES — 分歧与可疑点登记（合编）

口径：(a) 产出分歧 · (b) 表达受阻点 · (c) Zig 源码可疑底层问题。
分歧是成果不是失败：先登记、再分类（Zig 侧缺陷候选 / 本移植缺陷 /
规约含糊），**不许为对拍绿迁就改形**。分线登记在
`docs/divergences/<lane>.md`，本文件为协调线合编。

## 已知有意分歧（不迁就）

### K-1 [源 D-L2] lexer 转义末字节越界 —— Zig 侧缺陷候选（主会话已转主线登记）
- Zig `lexer.zig` 字符串分支与 `nextTemplate`：以 `\` 结尾的未终结串使
  index 走到 `buffer.len + 1`，invalid token 的 `loc.end > buffer.len`，
  下游按 loc 切片即越界（安全模式 panic / ReleaseFast UB）。
- Rust 侧钳制到 `buffer.len`（tag 与 start 不变，end 至多差 1）。
  对拍口径：涉及该形态的输入列为已知有意分歧，不算对拍失败。

## 子系统线 A（lexer/ast）合编 — 全文见 divergences/lexer-ast.md

| 编号 | 类 | 一句话 |
|---|---|---|
| D-L1 | b | 行注释后递归重启扫描 → Rust 改循环（行为等价，病理输入栈深差异） |
| D-L2 | c | 转义末字节 index 越界一字节（= K-1，Zig 缺陷候选） |
| D-L3 | a | 数字后缀吞食不对称（整数吞后缀、浮点不吞），镜像保留，禁单方面「修复」 |
| D-L4 | a | 非 ASCII 字节逐字节 invalid（每字节一 token），镜像保留 |
| D-L5 | c | 模板嵌套硬上限 8 且与词法错误共用 invalid tag，诊断面缺口候选 |
| D-L6 | c | `interp_depth: u16` 无溢出防护（理论项，两实现同构镜像） |
| D-A1 | b | AST 所有权：arena 指针网 → owned 树（裁决见 COORDINATION 决议 D-2026-1107-1） |
| D-A2 | b | 变体命名映射机械规则（snake_case → CamelCase 承载同名 payload） |
| D-A3 | a | Zig 字段默认值在 Rust 构造点显式化，刻意不伪造 Default impl |

## 协调线（ABI/宿主侧通读）登记

### D-C1 (b) Context 不是 C ABI，是双边手写镜像
- `plugin_api.Context` 为普通 Zig struct（非 extern），宿主
  `sci/src/plugins.zig` 另有一份同名定义；布局一致靠同源同编译器，
  无尺寸/版本握手（仅 descriptor 有 abi_version 校验）。broker 尾字段
  靠「追加保持前缀兼容」的注释纪律。
- Rust 侧以 #[repr(C)] 按字段序镜像；结论：这是规约含糊项——任何一侧
  改字段顺序都会静默错位，差分线后续可构造错位注入验证宿主校验盲区。

### D-C2 (c) handler ABI 的静默兜底：abiSlice 对 len==0 无条件返回 ""
- `plugin_handler.zig` `abiSlice`：`if (len == 0) return "";` 在指针
  为 null/任意值时都静默给出空串；`fields_len == 0` 时 fields_ptr 为
  null 亦被接受。属 AGENTS.md 点名的「静默吞错兜底」同族：参数契约
  的一半（指针有效性）在 len==0 时完全不查。
- Rust 侧实现将区分「空切片」与「null 指针 + 0 长」并响亮报错，
  届时与 Zig 的产出差异按 (a) 登记。

### D-C3 (c) sa_std 解析的硬编码机器路径兜底链
- `plugin_imports.zig` `resolveSaStdImport` 的候选根含
  `/home/vscode/projects/sci/sa_std` 与 `~/projects/sci/sa_std`：
  与当前机器无关的个人路径写死在发行代码里，解析失败时静默逐个试。
- Rust 侧只保留 SA_STD_DIR / SCI_ROOT / 祖先目录三条有据路径，
  硬编码个人路径不移植（登记为有意分歧候选，候裁决）。

### D-C4 (a) 输出 @import 列表不是源 import 的纯函数（待 codegen 线定层）
- 同为 demos 输入：03/04 的输出 import 列表为 7 行同一集合，
  01 多出一行 `sa_std/string.sa`（01 的 @test 体引用 str_eq，而 build
  产出中 @test 体被剪为空桩——被剪内容仍影响了 import 展开集）。
- 影响：切片对拍的 import 发射必须按「展开后程序」建模，不能按源文件
  import 列表直译。分类暂挂 (a)，归因候选：展开阶段按可达性收集时
  把 test 体的引用计入，而剪枝发生在展开之后。

### D-C5 (a) 全局计数器的两处不可见消耗（待 codegen 线定层）
- tmp 计数全程序连续，但 07 中 fact 结束于 tmp_6、add 却始于 tmp_8
  （tmp_7 被不可见地消耗）；标签计数同理：07 的 if 分配了
  THEN_0/ELSE_1 与未发射的 MERGE_2，@test 入口标签因此为
  L_TEST_ENTRY_3（03 中 MERGE 被发射、序号吻合；02/01 无标签消耗时
  TEST_ENTRY 为 0）。
- 分类暂挂 (a)：疑似 if 表达式结果槽/MERGE 标签先分配后丢弃；
  codegen 线复现时须给出确切分配点，否则按本移植缺陷处理。
