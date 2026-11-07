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

## CLI-D4 (b·覆盖→五期接通，残留三坐标) SAB 直降对 std 表面调用
基线形态（四期）：凡导入 std 表面（println 格式化、str_eq 等）的
程序，sab build / build-exe / test(sab) 在
`generate_from_program` 上返回
`SAB Error: direct SLA-to-SAB lowering failed: error.UnsupportedSabDirectFeature`。
根因 = std 片段需要 flattener/fragment 机器而 cli 未接
`generate_from_program_with_flattener`。

五期接通形态（本线，证据见 p5-deliver/d4/REPORT.md）：
- `sla-cli/src/pipeline.rs::compile_to_sab` 已切换到
  `generate_from_program_with_flattener`，provider =
  `sla-cli/src/sa_flattener.rs::SaHostFlattener`：经 delegate 的
  sa 解析路调宿主 `sa check <片段.sa> --emit-sab <出.sab>`
  （sci `executeCheck` = flatten+verify+encodeSabFromFlat，与
  Zig 的 flattenStdSnippet+verifier+encode 同链），产物以仓内
  `decode_module` 解码；import 片段源为 Zig 原文
  `@import "{path}"\n`，provider 内按路径缓存（只查不迭代）。
  sa 缺失/失败/产物异常一律响亮错误，无静默兜底。
- gen_call 的 println 臂已移植（`sla-codegen-sab/src/gen_print.rs`：
  genPrintlnCall/emitPrint* 家族 + planPrintlnArg 的字面量段、
  字符串字面量洞参、基元、借用/装箱基元计划）。
- 实证：`sla sab build fixtures/01_hello.sla` 成功，产物
  build-exe 运行输出 `hello world` rc=0；与 Zig 现头同件产物
  disasm diff 16 行，D4 辖区（deps extern decl、字符串常量、
  print 调用体）逐字全等，差在既有主干约定（L_ENTRY 缺失、
  SAB-D19 尾释放）——非本线辖区。
- 144 ms 异常同线定层修复：全在前端 load contracts 段（非 SAB、
  非委派）；根因 = sla-parser 导入宏注册表 Vec 按名线性查找 ×
  每条 EXPAND 行查询，对 ~6k 宏的 std 闭包二次膨胀。已改为
  `ImportedMacroRegistry`（records 注册序不变 + 名字索引只查
  不迭代），load contracts 331ms → 86–93ms（静窗实测）。

残留坐标（均响亮失败、不静默）：
1. **宏片段的 verified-encode 墙**：Zig 的模板/新鲜片段路用
   `encodeSabFromFlatUnchecked`，sa CLI 只提供 verified 的
   `check --emit-sab`；占位寄存器片段（如
   `EXPAND VEC_NEW __sla_macro_arg_0`）被宿主验证器以
   MemoryLeak 拒绝——凡经表面规则宏片段的 SAB 程序（Vec 方法
   等）仍在 provider 处响亮停（宿主 trap 原文上抛）。导入模块
   路（println/preload deps）全通。解法候选：sci 给 sa check
   加 unchecked encode 旗标，或 Rust 侧移植 flattener 本体。
2. **str_eq 臂未移植**：`sla test 01_hello.sla`（TestCodegen
   形态）死于 gen_call str_eq 桩（driver_gen.rs，D
   surface_call）；已隔离实证（去 @test 后 test 全通）。
3. **println 的 String 值计划未移植**：format_string /
   string_like 两计划在 gen_print.rs 内以计划名响亮止
   （STRING_BUF_AS_STR/CSTR_LEN 片段 + 字符串绑定物化分类
   契约未到）。

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
- **残余关闭（五期 CLI-D5R 线，2026-11-07，本条闭合）**：
  定层链坐实——泄漏寄存器 tmp_13/tmp_7 是测试函数内被
  `!=` 比较消费的 call 结果临时量；SAB 的 op/call/assign
  消费语义互不相同（op 不消费操作数、call 消费实参、
  assign 消费源），而 driver（B4 子集）从未接 E 组释放
  引擎（SAB-D19①），三个消费点全缺。修复 = 在
  driver_gen 的同形发射点接线（镜像 stock 反汇编）：
  ① 二元 op 后释放非本地操作数临时量（tmp 名 + 非本地
  双判据，for 循环变量式 tmp 名本地绑定不释放）；
  ② while 条件临时量在 body 与 cond_false 两后继头各发
  一条分支形 release；③ ReturnStmt 发射前经 E5
  DropEngine 消费 checker cleanups（releaseCleanupForStmt，
  与文本后端同点同表）；④ break/continue 经 E5
  gen_loop_jump 同形消费。验收：p_tests `sla test` 全量
  2 passed、--filter alpha 1 passed、无 MemoryLeak；
  build-exe 仍 rc=45；workspace 全绿（见 D5R REPORT）。
  既有 SAB 字节变化仅三处、均为释放补齐（指令级 diff
  核验为纯 Release 新增）：driver_e2e 的 P2 模型（+5 条）
  与 panic-guard TEST 模型（+2 条）重钉、driver_e2e_h 的
  if-value 手工模型补两条释放；P1/P3 与其余 fixture 逐字
  不变。残留登记不变：L_ENTRY 与标量落槽（SAB-D19②③）
  未动——stock 的落槽形在本子集以 assign 消费语义等价
  通过校验，落槽本体仍候 SAB 子集扩展时按需接线。

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

## 六期 R-2 销账（2026-11-07，rust-r2-fix 线）

- **CLI 展开接线（UNDEF-MODULE 91 跑根因）**：`run_frontend`
  此前对任何 FrontendKind 都不做 .sla 导入展开——展开实现
  （sla-plugin `sla_expand`）只接在 decl-only 的 check 形组合
  上，TestCodegen 形拿到未展开程序，模块函数全域
  `Undefined call`。现接线：仅 TestCodegen 形在解析后、CLI-D5
  剪枝前执行 include-all + 保留体展开（sla-cli 以 rlib 依赖
  sla-plugin——其 crate-type 本含 rlib，与 Zig 的插件/CLI
  拓扑一致，零代码搬移）；展开模块并入 freshness 闭包。
  Check/Compile 形的登记面按本线范围纪律**未动**。
- **导入宏登记（同根第二面）**：contracts 报告的
  `imported_macro_registry`（STR_PTR/VEC_RESERVE_GROW 等 SA
  宏）此前只有插件与 oracle 前端登记进检查器，CLI 只登记
  extern——test 体里一切宏调用 `Undefined call`。现于
  TestCodegen 形补登记（字段拷贝与 oracle 同形）。
  **Check/Compile 形的同一缺口在册**：普通 fn 体里的导入宏
  调用在 `sla check` 下仍 Undefined——与 R-1 同根，留 R-1
  后续线（本线范围纪律：不动 check 管线登记面）。
  【R-2 replay 合成订正（2026-11-08）】本节记述的是
  cc52e91 基线（rust-r2-fix 线）的状态。现头（3604444，含
  R-1 落地）上，R-1 已把同一登记循环对**全部形态无条件**接
  入 pipeline.rs——Check/Compile 形的导入宏登记缺口已闭合，
  不再在册；replay 树中 R-2 的 TestCodegen 限定登记循环因与
  R-1 循环逐字重复已被移除（见 rust-r2-replay/RESULT.md
  合成登记第 4 项）。
- **展开体解析回退（记录式）**：bodies 形展开中，模块体（或
  签名）含未移植解析面（impl 块、`&[T][owner]`、`T!`、go）
  时整展开失败会把仅传递导入该模块的程序一并拖死。现按模块
  回退 decl-only 并记入 `ExpandedProgram.body_fallback_modules`
  （不静默）：被调用的回退模块在检查期按签名解析、在发射期
  响亮失败；未被调用的模块不再阻塞程序。签名级缺口
  （impl/`&[T][owner]`/`T!`）decl-only 同样不可解析、回退不
  覆盖——那是 R-3 解析面本账。
