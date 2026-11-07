# CLI 命令层分歧登记（四期 sla-cli 线）

口径：三分类沿全仓惯例——(a) Zig 侧缺陷候选 / (b) 本移植缺陷 /
(c) 有意为之（含形态固有差异）。证据场地：Colab 盒
`/content/rust4/cli/parity*`（run_parity.sh v2/v3，run 3 =
parity3.log）；参照头 sla 2cc12ea8、sci 677b6a57；Rust 基线
d811673c。矩阵总账：49 记账行中 PASS 17、DIFF 32——DIFF 绝大
部分为下列已定类条目的逐案投影，语义通道（rc/stdout/stderr/
主产物）全等的案以类别注记为准，不逐案复述。

## CLI-D1 (c) help / --help 的宿主拦截形态差
`sa sla help` 与任意 `sa sla <cmd> --help` 在宿主侧被拦截：
宿主把 sap.json 的 help 字段（= 插件 HELP_TEXT）打到 **stdout**
且不调插件（A0-1 已证）。独立二进制无宿主拦截层，打印的是
插件自身的帮助路径（stderr、文本逐字同源；错误路帮助已活体
全等，如 check -p 案 PASS）。属形态固有差异，非缺陷；独立版
是否应把顶层 help 改投 stdout 属产品口径问题，候协调线裁断，
本线保持插件语义镜像不动。

## CLI-D2 (b) 解析诊断保真两处（sla-parser 辖区）
check/build p_syntax.sla 案 stderr 差两点：
1. Zig 解析器有一条 std.debug.print 侧信道行
   `Unexpected prefix token: semicolon at index 36`（D-P12 家族），
   Rust 解析器无此打印；
2. 期望描述：Zig 为 `valid expression or statement token`，
   Rust ParseError.expected 缺失时回退通用 `token`。
rc 与诊断主体（位置/源码窗/插入符）全等。坐标在 sla-parser，
非命令层可修。

## CLI-D3 (c→待镜像) Zig 侧 .sla-cache 辅助缓存副作用未镜像
stock 在编译时写两类辅助缓存文件，Rust 侧不产：
`.sla-cache/reachable/<file>-<wyhash>.plan`（可达性计划缓存，
SAB 路与 workspace build 出现）与 `.sla-cache/imports/*.lst`
（导入清单缓存，带 .sa 合同的程序出现）。主产物（.sa / .sab /
子进程 artifact）在这些案中逐字全等，属性能缓存层缺失而非
语义分歧；其格式属 plugin_reachability / 导入层辖区，本线
只登记不伪造。

## CLI-D4 (b·覆盖) SAB 直降对 std 表面调用覆盖不足（基线形态）
凡导入 std 表面（println 格式化、str_eq 等）的程序，sab build /
build-exe / test(sab) 在基线 d811673c 的
`generate_from_program` 上返回
`SAB Error: direct SLA-to-SAB lowering failed: error.UnsupportedSabDirectFeature`
（错误行形态与 Zig 同名错误镜像一致、响亮失败）。根因 = std
片段需要 flattener/fragment 机器（SAB-F 线 sabf.patch 已含
`StdSnippetFlattener` + `generate_from_program_with_flattener`），
集成时 sla-cli 应切换到该入口。无 std 导入的程序 SAB 路全通
（p_main42 build-exe 产物可运行 rc=42、p_tests sab build →
build-exe 运行 rc=45）。这是本线今晚最大的覆盖缺口，坐标
精确：sla-codegen-sab 直降入口 × std 表面调用。

## CLI-D5 (b) test-codegen 前置剪枝已移植；残余 MemoryLeak 再定层至 codegen-sab 释放发射
五期 CLI-D5 线（2026-11-07）处置结果：
- **阶段序已补齐**：`run_frontend` 的 TestCodegen 路径在 parse
  之后、contract load/check 之前，按 Zig 原序（读头
  sa_plugin_sla 2cc12ea8 的 plugin_compile.zig）落
  `pruneTestsByFilter` + `pruneUnreachableTestFunctionDeclsBeforeTypeCheck`
  （根集 = @test 体 / const 值 / trait impl 方法体 / overload
  方法体；传递闭包走 call 名、associated 候选、identifier、
  generic_func_ref 与 const/macro 引用根扫描；extern/decl-only
  声明恒保留），移植在 crates/sla-cli/src/test_prune.rs +
  test_reach.rs；check 之后旧有的 filter 剪枝已删（Zig 原文该
  函数仅前置一处）。非 TestCodegen 路径逐字不变。
- **复现形钉死**：p_tests.sla = while_sum 辅助函数（while 循环）
  + main 返回 while_sum(10) + 2 个 @test（一名含 alpha、无导入
  无 println 形），与原台账 tmp 编号逐字吻合（修复前 Rust
  全量 tmp_15 / --filter alpha tmp_9、rc=1；stock 2 passed /
  1 passed；普通 build-exe rc=45）。
- **残余再定层（本条未闭合）**：前置剪枝生效后（全量
  tmp_15→tmp_13、filter tmp_9→tmp_7，泄漏寄存器随 main 被剪
  而移位），SAB 测试输入仍被 sa test 以 MemoryLeak 拒绝。
  反汇编三方对拍定层：同一 checked 程序，文本后端
  （--test-backend sa）释放完整、2/2 PASS；Rust SAB 后端的
  while_sum 为纯 assign 形、**零 release**（局部量不落
  stack_alloc、临时量不释放），且普通 build 的 SAB 同形——
  `sa build-exe` 容忍该形（rc=45 活证）、`sa test` 的严格
  校验器不容忍；stock SAB 同函数为 stack_alloc+store/load+
  全量 release 形。故残余缺陷在 **sla-codegen-sab 的释放发射**
  （checker cleanups 消费面），与 test 配置无关、非管线阶段
  序问题；归属 codegen-sab 辖区，候协调线另派。
- 移植范围差（响亮登记）：Zig 的 SyntacticFactSet 常量事实
  分析未移植（known-false 分支剪枝不做、receiver 类型不由
  局部事实解析）——可达集为 Zig 的超集（不可解析 receiver
  时标全部 associated 候选，与 Zig 自身回退同形），不会多
  剪；模块表/imported macro 面在本管线无对应物（程序仅根
  文件）；rewriteProjectSnapshotTestShortcuts 未移植（保留名
  单已逐字镜像）；Zig SAB 尾的 check 后两道可达剪枝
  （post-typecheck syntactic + tc-based
  pruneUnreachableFilteredTestDecls）未移植，Rust 测试 SAB
  可能比 stock 多保留声明（不影响校验、影响字节对拍）。

## CLI-D6 (b·覆盖→已接通，五期 CLI-D6 线) sla_std 薄包根解析 + 正文衔接
新探针 p_sla_std.sla（`@import "sla_std/wrap/fmt.sla"`）：
stock check 通过；Rust 侧原报
`Import Error: failed to load @import contracts: import resolution failed: FileNotFound: sla_std/wrap/fmt.sla`。
定层（五期复现坐实）：contract_load 与展开层共用
`imports::resolve_import_files`，其 sla_std 祖先根解析本已在册；
真缺陷在祖先 walk 的起点——parity 案以**裸文件名**
（`check p_sla_std.sla`、cwd = 案目录、sla_std 树在上两级祖先的
`sa_plugin_sla/sla_std` 下）运行时，源文件 parent 为空路径，
`real_path("")` 规范化失败回退 `""`，walk 起步即终止、永不达
cwd 的祖先。直引 `.sai` 同病但被 parity 脚本的 `SA_STD_DIR`
环境根救回，sla_std 无此根故独红。
修复（共享 helper、一处）：`imports::real_path` 空路径归 cwd
规范路径，walk 由此获得真实起点；contract_load / sla_expand /
sla-cli 三消费方同受益、优先序与 imports.rs 原序不变（sla_std
根序 `sla_std` → `sa_plugin_sla/sla_std` →
`sa_plugins/sa_plugin_sla/sla_std` → `SLA_STD_DIR`）。
正文衔接：contract_load 扫描已解析 wrap 的正文、跟随其
`@import "sa_std/fmt.sai"` 再导入，包装层重导出的 extern
（sa_fmt_i64 / sa_fmt_buffer_len / sa_mem_free）以嵌套先行序
登记，visited 闭包 = [wrap/fmt.sla, fmt.sai]（freshness 闭包
口径不变）；与 sla_expand include-all 组合后（检查路 decl-only
+ 原始路带体）双路 check 均过。
双形式 fixture（硬验收）：同一 fmt 表面两探针经 Rust 管线
（sla-plugin 层：contract_load + expand + checker）双双通过、
extern 闭包逐字同——薄包形式
`crates/sla-plugin/tests/sla_std_wrap_pipeline.rs` +
contract_load 双形式闭包测试 + 裸文件名回归
`crates/sla-parser/tests/sla_std_bare_filename.rs`（修复前红：
同文 FileNotFound；修复后绿）。
残留（非本条辖区、如实登记）：sla-cli 二进制前端至今未调用
任何 `.sla` 正文展开（pipeline.rs 头注在册的未接线阶段，对
一切 `.sla` 导入同然、非 sla_std 独有）——其 check 案已由
Import Error 推进到 Undefined call 阶段；展开本体在
sla-plugin（六家拓扑定案：无人依赖 sla-plugin），cli 接线须
协调线裁断（展开下沉共享 crate 或授权新边），候裁。

## CLI-D7 (a·已在册) 函数尾释放序：Zig 非声明序（FB-020 家族）
build 02_arith / 07_functions 的 .sa 文本各差一行：尾释放序
stock 为 `!b !a !c !d`、Rust 为声明序 `!a !b !c !d`。与
FB-016/FB-020 在册事实同族（Zig 发射序受 HashMap 迭代影响、
非声明序 10/10），def-det-order 的定形方向（发射序 = 声明/
登记序）与 Rust 现行一致。本线不新占 FB 号，交叉引用在册条目。

## CLI-D8 (b) 无导入小程序 SAB 字节与 stock 在 offset 8 分歧
p_main42：managed 名（wyhash 路径）全等、stock 自比确定
（3ecd58… 两跑一致）、cli 自比确定（7216cc… 跨案一致），
两实现字节在 offset 8（cmp char 9）起分歧；下游
artifact.sa.bc 与 output.bin 逐字全等、产物运行 rc=42 全等。
坐标交 sla-codegen-sab 线以其字节 diff 工具定层（疑头部
标志/首节编码），命令层只登记。

## CLI-D9 (c) 新鲜度门的存储位置形态差（命中行为全等）
profile 双跑：stock 与 cli 第二跑均出现
`[sla-profile] freshness gate: hit` 且产物全等。差异仅在
存储位置：stock 经宿主 broker 存（工作目录内无
.sla-cache/fresh/），cli 按 Zig 源码自注的独立形态权威路径
走本地门（.sla-cache/fresh/*.json|bin，键构成/清单格式逐项
镜像 plugin_freshness.zig）。另 profile 阶段行 stock 打全量
子阶段行、cli 只打其阶段子集——诊断面覆盖差，不影响命中
语义。

## CLI-D10 (b·未接线) sab disasm 解码器缺失
解码本体在宿主 sci（sci_bridge.disasmSabAlloc →
sci/src/sab.zig disasmModule），本仓无 SAB 解码器。cli 镜像
读文件错误行（disasm_missing 案 PASS），对真实 .sab 响亮
拒绝并注明坐标，不伪造输出。候解码器移植线立项。

## CLI-D11 (c) 子进程测试输出排序噪声（非分歧）
test --test-backend sa 案：两跑之间 [PASS] 行序翻转过一次
（run2 全等、run3 序差），行集全等——sa test 子进程并发
排序所致，两侧同源。矩阵口径记为内容全等。

## FB 编号块 FB-049..FB-052 使用情况
**未使用**。本轮唯一 Zig 侧缺陷候选家族（尾释放序）已在册
为 FB-016/FB-020；其余分歧或为本移植覆盖/保真问题，或为
形态固有差异，均不新占号。
