# dc7-exec RESULT — D-C7 → C1 裁定的四件执行

本线只做裁定的四个小件（单测改写 / Haiku 附带条件探针 / 文档修订 / 语料新负例），
**未触碰 F5 消费门 `check_call_consume.rs` 任何行为**、未触碰 codegen、未触碰
Zig 树与 rust-r3-fix 树。全部提交只在本机仓 `~/workspace/dc7-exec`（克隆自
`~/workspace/rust-r6-fix`），无推送、无远端分支。

基点：本地链头 `23c737fa801733cab2e36a0c64ede47694516357`（R-6 RESULT 提交；
其代码域与远端 layola13/sla `7d66b464` 逐路径同）。命名说明：树根 `RESULT.md`
是 R-6 报告（克隆带入，未改动），本线报告另置 `RESULT-DC7.md`，与 R-6 落地
「RESULT.md 不入远端」的先例一致；落地是否纳入由主会话裁定。

## 0. 本地提交链（逐个声明对象存在性；主会话 cat-file 复核）

| sha（全） | 内容 |
|---|---|
| `7be1b859d60e206e73490906d5f4ed159d4b6943` | 件 1：单测改写 `crates/sla-checker/src/tests.rs`（+113/−13） |
| `f93ed5a87ab3594fc04b0e2ff8d8845a81427434` | 件 3：文档修订 `docs/divergences/checker.md` §D-C7（+37/−16，单文件单节） |
| `d87905238fb3dd9ab886429050ead039da30eb89` | 件 4：语料新负例 `corpus/tests/test_error_use_after_move_call_bare_read.sla`（+34） |
| `e6978f56a1c6923786763a340187822f26033a95` | 件 1 续：读先行用例断言修正（+6/−2，见 §1 说明） |
| 本提交的父提交之后的更正提交 | 本 RESULT-DC7.md（首版随 5886aa7 入库，本行 sha 以 `git log` 链头为准） |

五笔（含 RESULT 更正提交）全部 `git cat-file -t` = commit 已自查；链头全 sha 在终报中一并给出。

## 1. 件一：单测改写（VERDICT §4.1）

**改前**（链头 23c737f，`tests.rs:301-324`）：`plain_call_does_not_consume` ——
驱动 `check_source(...)` 并断言 Ok（D-C7 的非消费主义）；在 R-6 落地形态下该断言
与检查器行为冲突，是全工作区唯一红（基线 563 过 / 1 败）。

**改后**（新节标题 `// -- plain-call argument consumption (C1, ex D-C7)`，共 5 个
测试 + `plain_call()` / `h_decl()` 两辅助函数；`tests.rs` 总行数 589 → **689**
（≤700，未贴限，无需拆分）：

| 测试 | 形状 | 断言 |
|---|---|---|
| `plain_call_consumes_bare_read` | `fn h(s: String); fn f(s: String) { h(s); let b = s; }` | `Err(UseAfterMove)`——VERDICT §4.1 本体形（裸读 = `let` 隐式移动点） |
| `plain_call_consumes_second_call` | `h(s); h(s);` | `Err(UseAfterMove)` |
| `plain_call_consumes_compare_read` | `h(s); s == "x"` | `Err(UseAfterMove)`（与咨询包探针三 Err 读数一致） |
| `plain_call_consumes_return_use` | `h(s); return s;` | `Err(UseAfterMove)` |
| `read_before_call_then_error_at_call_site` | `let b = s; h(s);` | `Err(UseAfterMove)` 且消息含 "was already consumed" |

断言修正说明（提交 e6978f5）：读先行用例初版断言消息含 "call to `h`"，实跑该路径
先触发的是 `check_call_arg_consumption → check_identifier` 的通用文本
（`UseAfterMove: identifier \`s\` was already consumed`），因为 REG-BR-6 的
call 专属文本挂在标识符后续读取点而非实参检查点。按实跑文本改断言为 pin 实际行为，
错误落点（调用实参点）与 kind 不变——这是断言对文本的对齐，不是对行为的修改。

**工作区全量对账（任务书口径 564/564 的构成核对）**：链头 23c737f 的工作区总数
为 **567** 而非 564——564 是 R-6 链头 93693349 的读数，23c737f 的 RESULT 提交另带
3 个文档测试（`sla-oracle` 的 doc-tests）。改后实测
`cargo test --workspace --no-fail-fast`（串行低并行口径，日志 `ws_test.log`）：
**567 passed / 0 failed**（35 个测试二进制求和）；`sla-checker` 单包 238 过。
即：基线 563 过/1 败/共 567 → 改后 567 过/0 败；唯一败（D-C7 单测）被 5 个 C1
钉死测试取代（净 +4 于 checker 包：234 → 238）。

## 2. 件二：Haiku 附带条件探针（VERDICT §4.4）

临时探针置于仓外 `~/workspace/dc7-probes/`，**已全部删除、未入仓**；判读用
本线树构建的 release `sla` 二进制 + R-6 同口径环境（独立 `sla check`）。

**(a) preserved 调用后跟循环内字面量填充消费**：前段为 F7 既有语料件
`test_error_deep_copy_map_field_copy.sla` 的本体形（`idm(a)`，`a` 后续仍活 →
preserved 深度拷贝门）；后段为 `test_error_loop_consume_fill.sla` 的本体形
（循环外绑定 `seed` 在 `while` 内按字段字面量填充）。
判读：**首错（唯一错）= UncloneableCopy**（F7 门），LoopConditionalConsume 未出现。
机制：F7 门在 `check_call_arg_consumption` 内先于消费步骤（镜像 Zig
`checkCallArgsAgainstFunc` 的顺序，代码注释自证），首错即终止检查，填充段从未被
检查——这是**顺序先行**而非相互作用。分开判读确立：对照探针
`probe_a_ctrl_nocall.sla`（去掉 preserved 调用、期望改 LoopConditionalConsume）
实测**正是 LoopConditionalConsume**（"consumed by struct literal field fill inside
the loop body"），证明填充消费在 F7 门缺席时照常触发；两件既有语料各自单发
（R-6 neg_fix7 已录）亦为互证。**结论：无相互作用。**

**(b) 循环内按值调用消费 + 循环后使用**：`while i < 2 { sink(s); i = i + 1; }`
后 `len(s)`。
判读：**首错（唯一错）= LoopConditionalConsume**（"binding `s` cannot be consumed
by call argument inside the loop body"），与 R-6 §3/+1/Fix A 既定判读一致
（循环携带消费一律先响亮拒绝，Fix A 只豁免返回位置消费）。代码级机制：
F5 门与一切消费形共用 `consume_binding`（`checker.rs:456`），其循环守卫
（`checker.rs:469`）对循环体内非返回路径上、消费循环外绑定的**任何**消费形
（字面量填充 / push / insert / 调用实参）统一报 LoopConditionalConsume——
单一共享机制，无 F5 专属旁路。**结论：无相互作用，非新缺陷候选。**

附登记（不修、回主会话）：本线构建期本机出现两类场地伪影——EDQUOT 写拦截风暴
（git `tmp_obj_*` 垃
...[truncated 2112 chars]