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

## CLI-D5 (b) test-codegen 配置下 SAB 清理丢失（MemoryLeak）
p_tests.sla（while 循环 + 2 个 @test）：`sla test` 的 SAB 测试
输入被子进程 sa test 以 MemoryLeak trap 拒绝（live register
tmp_15 于全量、tmp_9 于 --filter alpha），rc=1；stock 同案
2 passed。同程序走普通 sab build → build-exe 运行干净
（rc=45 活证），故泄漏为 test-codegen 配置特有。定层坐标：
Zig runSlaFrontend 在 test_codegen 时于 checkProgram **之前**
做 filter 剪枝 + 可达剪枝，本线 pipeline 尚未移植该前置剪枝
阶段（见 REPORT 未移植阶段清单），checker 的 cleanup 数据与
剪枝后程序的节点对应关系在此错位，候协调线定辖（pipeline
阶段序补齐 vs codegen-sab 清理消费鲁棒性）。

## CLI-D6 (b·覆盖) sla_std 薄包形式尚不可用（14:39/14:40 规矩差距节）
新探针 p_sla_std.sla（`@import "sla_std/wrap/fmt.sla"`）：
stock check 通过；Rust 侧
`Import Error: failed to load @import contracts: import resolution failed: FileNotFound: sla_std/wrap/fmt.sla`。
两处坐标：(1) sla-parser 的 contract_load 解析器没有 sla_std
祖先根解析（imports.rs 的展开解析器有）；(2) wrap 是 .sla
本体，需要导入正文展开——IMP 线已在其 patch 接线，候集成。
旧形式 `@import "sa_std/xxx.sai"` 直引全程兼容（fixtures
01–07 全部以此形式活体通过），按补正口径：旧形式存在不算
差距，差距 = 薄包形式未通，本条即是。

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
