# DIVERGENCES — 分歧与可疑点登记（合编）

口径：(a) 产出分歧 · (b) 表达受阻点 · (c) Zig 源码可疑底层问题。
分歧是成果不是失败：先登记、再分类（Zig 侧缺陷候选 / 本移植缺陷 /
规约含糊），**不许为对拍绿迁就改形**。分线登记在
`docs/divergences/<lane>.md`，本文件为协调线合编。

编号命名空间（防撞号）：`D-C1..D-C5` 是协调线（ABI/宿主侧）条目；
checker 线的同名 `D-C1..D-C10` 是另一个命名空间，合编中一律写作
「checker D-Cx」，其头号条目另有主线候选名 **D-CK1**；parser 线为
`D-P1..D-P13`，SAB 线为 `SAB-D01..D16`，lexer/ast 线为 `D-L*`/`D-A*`，
lowering/text 线为 `D-T1..D-T14`（全文 divergences/lowering-text.md）。

反馈环（用户 2026-11-07 常设政策）：分类为 Zig 侧缺陷候选且定案者，
当轮在 `docs/feedback/` 落反馈单，主会话据此派 Zig 侧修复线与移植
并行；Rust 侧永远只镜像 + 登记，不替 Zig 改行为。已发反馈单：
FB-001 = D-CK1、FB-002 = DEF-LEX-ESCAPE-EOF（均已实证确认）；
FB-003..FB-009 = lowering/text 线 (c) 类七条 D-T9/D-T3/D-T4/D-T5/
D-T6/D-T7/D-T8（源码机制定案，候主会话确认派线）。
二期续发：FB-010..FB-014 = checker 扩围五条（D-C12/D-C13/D-C14/
D-C15/D-C16）；FB-015..FB-017 = SAB 二期三条（D17/D18/D22）；
FB-020 = oracle 二期 D-O1（checker D-C6 的字节级实证）。

**二期合编（2026-11-07）**：checker +7（D-C11..D-C17）、SAB +6
（SAB-D17..D22）、oracle +2（D-O1/D-O2），在册合计 **82 条**；
集成发现 INT-1 已销账（见文末），INT-2 仍开口。

**三期合编（2026-11-07）**：SAB +6（SAB-D23..D28，sab.md 总账
28 条）、checker +6（checker D-C18..D-C23）、合同段新开
contracts.md +8（INT2-1..INT2-8）、A0 命令面新开 a0-commands.md
+4（A0-1..A0-4），三期新登记 **24 条**，在册合计 **106 条**。
销账：**INT-2 已关闭**——合同段（D-A2 归位）+ INT21 名字特判臂
落地，5/5 fixtures 全管线 rc=0、01/03/04 逐字节全等、02/07 仅
剩 D-C23（= D-C6/FB-020 的端到端实形）；INT2-1 随之销账，
INT2-2/INT2-3 经实证裁定不阻塞（INT2-3 的 Zig 侧 layout_defines
表全仓无读者），INT2-4/INT2-5/INT2-7 与 `.sla` 正文展开为响亮
未接线剩余候四期。三期续发反馈单：FB-025（SAB-D25，const 折叠
SIGFPE 真产物实证）、FB-026（SAB-D28 家族）、FB-035..FB-038
（HND 评估四单，FB-035 = 第二 ABI 未导出，重大）；FB-021..024、
FB-027..034 经各线核查无单可立、号段归还（无单亦为结论）。

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

### D-C4 (a) 输出 @import 列表 —— **已结案**（lowering/text 线 D-T1 定层）
- 原疑问：01 比其余 fixture 多一行输出 import，疑似与 @test 体可达性
  有关（被剪的 test 体引用仍计入展开集）。
- **裁决：假设不成立。** 发射导入 = 程序（展开后）导入声明按序 + 5 条
  固定导入（string.sa、string_format.sa、io/print.sai、fmt.sai、
  core/panic.sa）。01 多一行仅因其源码有第 3 条导入
  （`sa_std/string.sa`）；其余 fixture 均为 2 条源导入 → 7 行。
  导入列表是源导入的纯函数，与 test 体无关。
- Rust 已按此模型实现（program.rs），fixture 01 逐字节通过、02/03/
  04/07 亦于 2026-11-07 在场地三复验全等（5/5）。

### D-C5 (a) 全局计数器的两处不可见消耗 —— **已结案**（lowering/text 线 D-T2 定层）
- `tmp_7`：codegen.zig:15871 —— 无结果槽的 if（void if）在 genExpr
  末尾 `return try self.newTmp()`，分配一个从不发射的临时寄存器
  作为丢弃结果；fact 的 if 双分支均 return，add 因此从 tmp_8 起号。
- `L_MERGE_2`：codegen.zig 约 15744 —— 普通 if 路径无条件
  `newLabel("L_MERGE")`，仅当 `needs_merge` 为真才发射标签行；
  fact 的 if 双分支终止且有 else，标签被分配但不出现，test 入口
  因此是 L_TEST_ENTRY_3。
- Rust 已逐字镜像两点（expr_if.rs，含丢弃 tmp 的分配）。本条自
  「待定层」销账：不是缺陷，是 Zig 的确定性计数器行为，差分须知。

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
| checker D-C11 | b | 【二期】泛型实例化不在检查器——归 monomorphizer 的流水线分工（镜像「不存在」本身） |
| checker D-C12 | c | 【二期】解构消费无 Copy 豁免：let 解构根绑定无条件消费，Copy 元组亦然；FB-010 |
| checker D-C13 | c | 【二期】switch 模式零检查（模式本身不经类型/穷尽校验）；FB-011 |
| checker D-C14 | c | 【二期】generic_func_ref 裸 CompileError、无诊断文案；FB-012 |
| checker D-C15 | c | 【二期】pattern 的 writable 标记读作 is_const（语义反转）；FB-013 |
| checker D-C16 | c | 【二期】match 推断路 OR 条件 + 回写 scrutinee 类型（检查改写被检对象）；FB-014 |
| checker D-C17 | b | 【二期】侧表键口径：Zig 按 CallExpr 子对象指针、Rust 按节点地址；清理名单两边同为名字列表（与 D-CK1 的关联面） |

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

计数：一期 (a) 1（SAB-D01，已修正复测全同）· (b) 5（D06/D08/D09/D15/D16）·
(c) 10（D02/D03/D04/D05/D07/D10/D11/D12/D13/D14，其中 D10/D11 标嫌疑候二期定性）；
另已核查排除 4 项。二期续登 D17..D22 后全册 (a) 2 · (b) 8 · (c) 12，合计 22。要点摘录：

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
| SAB-D17 | a | 【二期】sig.id 双轨：genFuncSig 用发射计数（:5964，两表长度和、入表前赋值，test 双计数跳号）、declInstForSig 用 intern id（:6008）——**已逐字镜像并以真产物字节级验证**（multi.sab sig.id 0/1/2 vs 操作数 0/5/9；multi2.sab 跳号 0/1/3）；Zig 侧气味 FB-015 |
| SAB-D18 | c | 【二期】逃逸闭包按 AutoHashMap 迭代序（指针哈希序）发射——产物字节序本质不确定；端口以插入序保存；FB-016 |
| SAB-D19 | b | 【二期】driver 子集省略面登记：E 组释放/清理全缺、无 L_ENTRY（模型口径）、标量重赋不走栈槽、while 分支状态恢复未接（if 值形态已响亮拒绝） |
| SAB-D20 | b | 【二期】B2 所有权形态：StdImportModule 存四段内容、MacroLocalChange.old 为 Option、宏 ctx 为 Rc 句柄 |
| SAB-D21 | b | 【二期】test decl 命名口径（Zig 引号名；一期模型名系占位）；附：test sig 的 llvm_name `_saasm_test_{id}` driver 尚未设置，列三期项 |
| SAB-D22 | c | 【二期】panic 非单参数形态静默写死代码 "1"、多参实参不求值被丢弃；driver 字节镜像；FB-017 |

对拍口径注记（SAB 线交付声明，协调线采纳）：一期 SAB 对拍 =
**同模型对同模型**（编码器 + emit 主干对手工 lowering 的 fixture01 模型，
与 sci 现头原编码器探针 cmp 全同：76B sha 000fb0e6…、427B sha 31fc5e86…）；
fixture01 的 println/str_eq 走 std 表面展开属二期，**01 端到端不列一期判据**；
AST 入口现响亮报 UnsupportedSabDirectFeature。
**二期更新**：AST 入口 `generate_from_program` 已接 B4 driver 真实驱动
（B2/B3/B4 落地），fixture01 级非 std 表面程序端到端发射并与手工模型
逐字节相等（334B sha e92ab628…→双轨镜像后重钉 2ab94012…；控制流件
1184B）；std 表面（D 组）与 C/F/G/H/I 未接线调用点一律响亮报错带
组别 + Zig 行段。二期探针复验：76B/427B sha 维持全同。

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

## 子系统线 D（lowering/text）合编 — 全文见 divergences/lowering-text.md

参照源：lowering_rules.zig（9,705 行）、codegen.zig（16,288 行）、
control_flow_rules.zig。对拍状态：fixture 01 先行全等，02/03/04/07
两处根因（D-T10 的实参 clone 破地址键、walker 未登记字面量/标识符
类型）修复后，2026-11-07 场地三复验 **5/5 逐字节全等**
（`cargo test -p sla-codegen-text --test fixture_parity` 全 ok）。

- **D-T1 (a) = D-C4 结案**：导入列表是源导入纯函数（+5 固定），见上。
- **D-T2 (a) = D-C5 结案**：tmp_7 / L_MERGE 两处确定性计数器消耗，见上。
- **D-T3 (c) ★ `sum` 硬编码跳过实参释放**：codegen.zig:9830-9832，
  `emitLoweredCallArgCleanups` 对名为 sum 的调用整段跳过全部实参
  释放，无注释无支撑；同名用户函数同样中招（名字碰撞改语义）。
  静默兜底族。→ **FB-004**。Rust 逐字镜像候裁决。
- **D-T4 (c) 未知被调用名静默假定宏 EXPAND**：codegen.zig 约 15540，
  函数表/外部表/内建全未命中时不报错、直接发射 EXPAND；拼写错误
  静默变宏展开，错误推迟到汇编/链接层甚至永不暴露。→ **FB-005**。
- **D-T5 (c) println 未知类型实参只打印 1 字节**：codegen.zig 约
  2397-2417 兜底臂发射 `sa_print_bytes(&{ptr}, 1)`，无诊断，
  格式化输出静默截断为单字节。→ **FB-006**。
- **D-T6 (c) println 长度口径不一致**：字面段用原始跨度（约 2185）、
  字符串实参用转义后长度（约 2195）；格式串段含转义序列时段路径
  多打字节。→ **FB-007**。
- **D-T7 (c) @test 空壳来自流水线前置裁剪**：genTestDecl（7610）
  忠实生成所给体；golden 中空体是 plugin_compile 构建模式在
  codegen 前清空 test 体所致——构建产物里的 @test 只是空壳，
  「产物即已测」的读法会高估实际执行断言数。流水线口径问题。
  → **FB-008**。接线注记：对拍台以空体喂入匹配 build 模式 golden，
  清空动作属流水线（oracle）职责。
- **D-T8 (c) 死分支（轻微）**：lowering_rules.zig:6221-6226
  `shouldAutoBorrowReceiverArg` 内层 borrow 判定后无条件 return
  true，判定对结果无影响，疑似重构残留。行为等价，仅登记。
  → **FB-009**（严重度：琐细）。
- **D-T9 (c) ★★ genStmt 静默吞未处理语句**：codegen.zig:10650 的
  genStmt switch 以 `else => {}` 收尾——未覆盖语句种类不产出、
  不报错，程序静默缺失整段语句。与 SAB 吞错族、主线「禁止静默
  兜底」同科，本批最重。→ **FB-003**。Rust **有意不迁就**：
  未覆盖语句响亮报错（D-T12），对拍口径下触发即红。
- **D-T10 (b) ★ CheckInfo 地址键契约**：Zig 以 `*Node` 为键的
  检查器侧表，在 Rust 显式化为 `sla_lowering::CheckInfo` trait
  （键 = node_key，与 checker 线地址公式逐字一致）。**接线契约：
  喂入 codegen 的树必须与登记事实时是同一棵、其后不得 clone/
  搬动节点**——一处实参 clone 曾使 println 实参丢失类型事实、
  落入 D-T5 的 1 字节兜底臂（已改全程借用）。oracle 接线现合此约。
- **D-T11 (b)**：hoisted_allocs 由参数穿线改发射器状态，产出不可见。
- **D-T12 (b)**：未移植发射域（for/switch/if-let/字面量族/宏族/
  async/dyn 等，清单在 sla-lowering/src/deferred.rs 的
  DEFERRED_FAMILIES）在 Rust 侧一律响亮 Unsupported（含 Zig 行号
  参照），不产部分文本——与 D-T9 对应的有意分歧。
- **D-T13 (a) 风险登记**：浮点常量格式化未验证（Zig `{d}` vs Rust
  Display 最短回环）；切片内无浮点未触发，含浮点 fixture 落地时
  须专项对拍。
- **D-T14 (a) 归因不可分辨**：fixture 04 出口清理（!i/!limit）挂
  while 尾随清理表或 return 清理表字节完全相同；本线对拍台挂在
  return 上。真实归属候 checker 线边界转储对账（若不同，仅测试台
  事实登记调整，发射代码不变）。

## oracle 线二期合编 — 全文见 divergences/oracle-p2.md

| 编号 | 类 | 一句话 |
|---|---|---|
| D-O1 | c | return 清理序：Zig `!b !a !c !d`（哈希序）vs Rust 声明序——**checker D-C6 的首个字节级实证**（e2e_a 全文仅此一处 diff）；非 Copy 类型 drop 序可观察；Rust 按纪律不迁就哈希序；FB-020 |
| D-O2 | a | oracle 未接 build 模式 @test 体清空（D-T7 定层的流水线职责）——**二期收尾已接线销账**：e2e_b 逐字节全等（sha 8623e8d0…）、e2e_c 维持全等、e2e_a 仅剩 D-O1 |

二期全管线对拍结论：e2e_c 与真插件产物**逐字节全等**（sha fe190281…，
实验首个 AST 驱动端到端对拍点）；5/5 fixtures 被 INT-2 挡在 check 段
（println 调用无从注册，逐件清单见 oracle 线报告 §3）。

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
- **二期已销账（2026-11-07）**：根因 = Rust 侧 `check_tail_type`
  自创了 Zig 不存在的 missing-return 判定（只认尾 expr_stmt、
  尾随 return_stmt 即报缺失）；Zig `checkFunc`（type_checker.zig
  :2596-2604）根本无此检查（仅四条件核对尾表达式类型，return 值
  由 return 臂 :2991 自行核对）。已按 Zig 原形重写，真值双路
  （源码 + 场地真插件 9 探针逐条对齐）；checker 24/24（+8 条
  parse→check 形态单测，含钉住「非表达式尾/空 body 放行」的
  Zig 真实口径）；oracle 三探针 e2e_a/b/c 转 rc=0，全管线对拍解锁。

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
- **二期状态（2026-11-07）**：无 std 依赖对拍已完成（e2e_c 全等、
  e2e_b 全等、e2e_a 仅剩 D-O1）；本条仍开口——阻挡清单已逐件确立
  （5/5 fixtures 全死于 check 段 `println` 调用、rc=5，01 另有
  `str_eq` 第二挡，oracle 线报告 §3），候三期立项 .sai 合同解析 +
  展开接线（与 SAB D 组表面预载同批规划）。
