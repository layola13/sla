# DIVERGENCES — 分歧与可疑点登记（合编）

口径：(a) 产出分歧 · (b) 表达受阻点 · (c) Zig 源码可疑底层问题。
分歧是成果不是失败：先登记、再分类（Zig 侧缺陷候选 / 本移植缺陷 /
规约含糊），**不许为对拍绿迁就改形**。分线登记在
`docs/divergences/<lane>.md`，本文件为协调线合编。

编号命名空间（防撞号）：`D-C1..D-C5` 是协调线（ABI/宿主侧）条目；
checker 线的同名 `D-C1..D-C10` 是另一个命名空间，合编中一律写作
「checker D-Cx」，其头号条目另有主线候选名 **D-CK1**；parser 线为
`D-P1..D-P13`，SAB 线为 `SAB-D01..D16`，lexer/ast 线为 `D-L*`/`D-A*`。

反馈环（用户 2026-11-07 常设政策）：分类为 Zig 侧缺陷候选且定案者，
当轮在 `docs/feedback/` 落反馈单（FB-001 = D-CK1、FB-002 = DEF-LEX-ESCAPE-EOF，
均已实证），主会话据此派 Zig 侧修复线与移植并行；Rust 侧永远只
镜像 + 登记，不替 Zig 改行为。

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
- **实测补注（2026-11-07，协调线 abi_probe，证据 docs/probes/abi_probe.out.txt）**：
  Context 实际 **72 B**（ABI_MAP 的「约 88B」作废）；且 Zig 自动布局
  **重排了字段**——物理偏移：allocator 0 / host_version 16 / log 32 /
  log_ctx 40 / **broker_call 48 / broker_ctx 56 / broker_abi_version 64 /
  json_mode 68**，与声明序（json_mode 第 5、broker_abi_version 第 6）
  不同。Rust 镜像已按实测序落盘并以偏移断言钉死（sla-abi 测试 5/5）；
  按声明序镜像会同尺寸、错偏移——正是本条预警的静默错位形态。
  附带教训：可空指针必须镜像为裸指针，`Option<*mut T>` 在 Rust 里
  带判别标签（16 B），第一版镜像因此整体膨胀，被布局测试当场抓出。

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

## 子系统线 B（parser）合编 — 全文见 divergences/parser.md

| 编号 | 类 | 一句话 |
|---|---|---|
| D-P1 | a | trait/impl/@overload 切片未覆盖，响亮拒绝（本线未覆盖，非 Zig 分歧源） |
| D-P2 | a | 模板字面量未覆盖，响亮拒绝；块跨度一致性随模板覆盖复核 |
| D-P3 | a | .sla import 预扫描显式化：parser 只记模块名，类型名由 oracle 经 seed_known_type_names 注入（协调线已接，见 COORDINATION） |
| D-P4 | b | 试探解析回滚：Zig 靠 (lex,tok) 值快照，Rust 预扫全 token 流、快照退化为下标 |
| D-P5 | b | 解糖别名同一 *Node（复合赋值 target、`?.` 的 cond 等）→ owned clone 后指针同一性消失；checker 线已以 BindingId 身份键控自带处理（销账） |
| D-P6 | b | 函数体跨度侧表：指针键 → 解析顺序表（Vec 元素地址不稳定） |
| D-P7 | b | 块跨度：Zig 在源码字节重跑镜像 lexer 数括号，Rust 在 token 流上计数 |
| D-P8 | b | 错误形态：Zig 枚举 + last_expected 侧信道，Rust ParseError 自带文案 |
| D-P9 | c | parseInfixExpr 的 .bang 宏调用分支疑似不可达（被 parseExpr 的 postfix-! 拦截遮蔽）★ 候主线确认宏调用是否走前置展开 |
| D-P10 | c | 三元试探 catch 吞真实解析错误、降级二次报错，诊断位置/形态可失真 |
| D-P11 | c | import 预扫描在解析器内做文件 IO 且失败被 catch {} 静默吞没；解析结果随环境变（与 D-C3 同族） |
| D-P12 | c | parsePrefixExpr 兜底分支 std.debug.print 直写 stderr，绕诊断通道、无开关 ★ |
| D-P13 | c | ParserError.Overflow 声明未抛出：超范围整数误报 InvalidCharacter ★ |

## 子系统线 C（checker）合编 — 全文见 divergences/checker.md

| 编号 | 类 | 一句话 |
|---|---|---|
| checker D-C1 = **D-CK1** | (b)+(c)→(a) | 分支/循环状态快照按名字键控、遮蔽绑定被合并 → 条件 use-after-move 漏检。**协调线已全管线实证确认（下节）**；FB-001 |
| checker D-C2 | b | 诊断 = 错误联合标签 + 1024B 定长缓冲 + 包装器兜底文案；命名子诊断只能以消息前缀承载（Loop/MacroConditionalConsume） |
| checker D-C3 | b | 类型图是可变 arena 指针网、五张侧表按节点指针键控；Rust 以 owned 值 + 借用期地址键，检查中改写节点无对应写法 |
| checker D-C4 | b | 循环检查无不动点 + 循环上下文双通道记账（字段与参数穿线并存） |
| checker D-C5 | (b)+(c) | 返回类型借假符号 return_ty_sentinel 走私，多处 .? 读回——漏守约定即编译器 panic 而非诊断；符号污染作用域遍历 |
| checker D-C6 | c | 块退出清理名单顺序 = StringHashMap 哈希迭代序（顺序敏感性未证实，候主线核 codegen drop 顺序依赖） |
| checker D-C7 | b | 「消费」是枚举的站点清单而非转移规则：普通调用按值实参在检查器层不消费，归属由 lowering 的 DEF-5 物化决定——账本本身回答不了归属 |
| checker D-C8 | b | 两套类型相等并存（默认宽度不敏感 / 严格版），严格只打在出过事的调用点，靠自觉选用 |
| checker D-C9 | c | while 条件臂一条裸 return TypeMismatch 无 setError 文案，用户所见取决于包装器兜底（低危诊断面缺口） |
| checker D-C10 | a | extern 实参类型整体旁路（源码注释明言 bypass FFI 签名检查）——有意设计，显式登记备查 |

### D-CK1 实证结论（协调线全管线复现，2026-11-07）

- 环境：Kaggle 2201，场地插件树 `699f13d4`，`sa sla check`（dev 模式）。
  输入三件：`docs/probes/dc1_{floor,control,shadow}.sla`，
  原始输出 `docs/probes/dc1_repro.out.txt`。
- 结果：floor（无条件双移）→ UseAfterMove rc=1；control（条件消费、
  **无遮蔽**）→ UseAfterMove rc=1；shadow（checker D-C1 最小输入、
  内层 `s` 遮蔽参数 `s`）→ **rc=0 放行**。唯一变量是遮蔽，
  与名字键控快照机制（saveScopeStates :2712 由内向外覆盖、
  restoreScopeStates :2725 写回每一层、then 臂内联复制 :5366、
  mergeMultiBranchScopeStates :2738 按名读）的预测逐项吻合。
- 复现注记：原文最小输入在 Zig parser 处需给 if-else 语句尾补 `;`
  （无分号形态先报 SyntaxError），协调线复现件已补，语义场景不变。
- **结论：Zig 侧缺陷成立（非证伪）**。反馈单 FB-001 已发；
  修复要点（建议）：快照/合并/清理三处一并改绑定身份键控，
  只改一处仍会按名释放错对象。

## 子系统线 E（SAB）合编 — 全文见 divergences/sab.md

计数：(a) 1（SAB-D01，已修正复测全同）· (b) 5（D06/D08/D09/D15/D16）·
(c) 10（D02/D03/D04/D05/D07/D10/D11/D12/D13/D14，其中 D10/D11 标嫌疑候二期定性）；
另已核查排除 4 项。要点摘录：

| 编号 | 类 | 一句话 |
|---|---|---|
| SAB-D01 | a | **参照树版本漂移**：本地 sci 滞后现头致签名记录差 1 字节；已按现头 e82d2865 修正、探针对拍 sha 全同。教训：差分基准必须对版（REPORT 已留笔） |
| SAB-D02 | c | std 表面文件定位多根回退链整段 else \|_\| {} 吞错——定位失败与「用了另一份 std」不可区分（与主线 SAB 静默测旧 std 同族） |
| SAB-D03 | c | 结构体字面量/更新路径 defer 中 releaseExprResultIfNeeded catch {} 吞释放错误 |
| SAB-D04 | c | emitBalanceReleaseLocal 取不到类型即 catch return 静默跳过平衡释放——恰在最需响亮报错处 |
| SAB-D05 | c | 拷贝派生缓存多处 catch {}：推导失败与无派生合并为同一结果 |
| SAB-D07 | c | 不支持原因默认不可见：traceSabUnsupported 靠环境变量且其读取自身 catch 吞错；失败响亮、原因默认哑（端口以 SAB-D15 对治） |
| SAB-D10 | c | 布局查询把「类型不可映射」与「无此字段」合并为 null 返回，下游不可分辨（与 D14 历史案型同构，嫌疑） |
| SAB-D11 | c | futureReadinessForState 状态缺失 orelse .unknown 兜底继续发射（嫌疑） |
| SAB-D12 | c | 检查器无类型时 codegen 按字面量形状伪造类型继续发射（exprTypeOrFallback）——漏类型从响亮失败变潜在错码 |
| SAB-D13 | c | emitBranch 把 then 标签重复写入 operands[2]（sci 消费者只读 [1]/[3]，今日死数据）：**字节级原样复现**，oracle 逐函数字节比对时是已知复现点、勿误判本端口缺陷 |
| SAB-D14 | c | 布局吞错家族模式登记（历史 SIGSEGV 案型 + 当前活实例 D02/D04/D10），二期逐模块普查 else \|_\| / catch {} / orelse 字面量 |
| SAB-D06/D08/D09/D16 | b | 错误通道（catch unreachable/@intCast 面）、寄存器身份、所有权状态集、单 struct 733 方法→50 模块等结构性重构，见全文 |
| SAB-D15 | b | 端口错误无条件携带原因字符串（对 D07 的有意分歧，仅诊断面） |

对拍口径注记（SAB 线交付声明，协调线采纳）：一期 SAB 对拍 =
**同模型对同模型**（编码器 + emit 主干对手工 lowering 的 fixture01 模型，
与 sci 现头原编码器探针 cmp 全同：76B sha 000fb0e6…、427B sha 31fc5e86…）；
fixture01 的 println/str_eq 走 std 表面展开属二期，**01 端到端不列一期判据**；
AST 入口现响亮报 UnsupportedSabDirectFeature。

### sleb128 已知向量分歧 — 结案（COORDINATION §6 销账）

- 裁决：**本移植缺陷（测试向量笔误）**。原向量 `[179, 159, 127]` 是
  -12365 的编码，被误作 -12345 的期望；Rust 实现产出 `[199, 159, 127]`
  与 Zig 原编码器逐字节一致。
- 证据（协调线场地探针，docs/probes/sleb_probe.out.txt）：原 sci
  `encodeSleb128`（场地树）对 -12345 输出 `c7 9f 7f`、对 -12365 输出
  `b3 9f 7f`，与解码验算互证；SAB 线自查结论与此一致。
- 程序注记（如实登记）：round2 失败日志写于 00:52:27 UTC、测试向量
  于 00:52:58 被改（31 秒后）——改在书面裁决之前，程序上先行了一步；
  但改动方向经独立探针证明正确、实现本身从未被迁就修改，
  故不构成「改测试迁就实现」的实质违规，仅记程序瑕疵备查。

## 协调线集成发现（2026-11-07，全管线接通后）

### INT-1 (本移植缺陷，checker 线) 非 void 函数的 return 在场仍报 missing return
- 最小复现（docs/probes/e2e_c.sla，全文 3 行）：
  `fn one() -> i32 { return 1; }`
- 现象：oracle 边界转储证明 tokens/AST 全对（body = [ReturnStmt
  { value: Some(Literal(IntVal(1))) }]），checker 仍报 CompileError
  "missing return value in non-void function `one`"（rc=5）。
  五件 fixtures 与无导入探针 e2e_a/e2e_b 全部死在此处。
- 定层：parser↔checker 的 AST 契约无误（转储为凭），问题在 checker
  的返回在场判定（check_func 的终结/尾值分析路径）内部；checker 线
  16 个单测全绿但未覆盖「源码 parse → check 普通 return 函数」形态。
- 处置：归 checker 线修复（其单测集补此形态）；协调线不代改。

### INT-2 (本移植覆盖缺口，一期范围) 合同加载/导入展开段在 crate 图中无主
- Zig 对应面：plugin_import_expand 的 loadImportedContracts（.sai
  合同 → extern 注册表）+ prescan；checker D-C10 已注明 extern_funcs
  「仅由 .sai 合同文件解析填充」。
- 现象：fixtures 的 println 等 std 表面调用在 checker 处无从注册
  （01 号诊断 "checkExpr failed at call println with 1 args"）；
  一期 10-crate 图没有任何 crate 拥有这一段，parser 只记模块名、
  checker 只接受注册、oracle 的 seed 只覆盖 .sla 类型名。
- 处置：二期/集成后续立项（.sai 合同解析器 + 展开接线）；INT-1 修复后
  无 std 依赖的程序应先行对拍，本条只挡含 std 表面调用的 fixtures。
