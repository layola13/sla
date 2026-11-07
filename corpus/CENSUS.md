# Rust 独立二进制首轮全量对拍普查（CENSUS · 六期第一线，2026-11-07）

- 对象：Rust 仓 `sla` 独立二进制（源头 `d59608c` 合并树 release 构建，
  本地提交 `ffdc1cd` 的 corpus 装配，盒上 write-tree 与本地提交逐字一致）
- 语料：本目录镜像（sa_plugin_sla `753fd682` 的 tests/ + demos/，4415 文件，
  聚合哈希双侧对账一致，见 MANIFEST.tsv）
- 场地：Colab 盒，隔离 HOME（plhome-rust6）；env 镜像 langgap 门 runner 约定
  （SA_STD_DIR 钉 sci `e0f1bc14` 的 sa_std、SLA_STD_DIR 钉源头 sla_std、
  SA_PLUGIN_DEV=1、SA_SLA_FIXTURE_DIR 钉 corpus 的 import_fixtures）
- Zig 参照：langgap 落地门全扫 stock 臂失败名集（run 级 16 条、check 级 161 名，
  原件在盒 /content/lgland/sweeps/）+ demos 普查基线 1251/1258
  （sla `13b7c927` 时点，F-1..F-5 + F-8 六名）
- 判读：四联口径（rc + `test result: ok` 结果行 + 失败名集 + 双后端），
  与门的全扫判读函数逐字相同；负例只走 `sla check` 判诊断；
  全部判读取日志原件。**本线只测不修，未改任何 Rust 源码。**
- 对照实验（证伪装配工件）：五期自有二进制（/content/rust5/final，
  与本轮构建同字节数）在五期 env 下对最小复现同败；换旧 std 对照同败。
  下列缺口均为交付代码实据，非本线装配问题。

## 1. 分母

| 面 | 分母 | 说明 |
|---|---|---|
| tests run 级 | 567 文件 × 双后端 = 1134 跑 | 门名单 564 + lang-gap 新增 3（fallible 两件 + dns_client_d） |
| tests @test | 2513 个 | 567 文件内 @test 总数 |
| tests 负例 | 151 文件 | test_error_* 116 + neg_* 35 |
| check 级 | 2022 个 .sla | tests 747 + demos 1275（门名单 2005 + 新增 17） |
| demos | 1258 单元 | basic 925 / rosetta 315 / syntax_sugar 5 / cxu 12 / workspace 1 |

## 2. 结果总表（三分类）

### run 级（1134 跑）
- 双侧同绿：PASS 跑 23；**整文件双后端全绿仅 7 文件**，单后端绿 9 文件。
- 双侧同红：16 跑（= 门 stock 失败集全数，存量不归 Rust）。
- **Zig 绿 Rust 红（真缺口）：1095 跑**。@test 通过数：sa 17 / sab 19（分母各 2513）。

### check 级（2022 文件）
- Rust 绿 1045（51.7%）。门名单 2005 内：双侧同红 144、Rust 独红 **821**、
  Zig 独红 17（其中主体为 Rust 反而放行的负例，见 R-6）。

### 负例（151）
- 响亮拒绝 131（86.8%），其中带具名诊断 kind 的 68、未具名 63；
  **被 Rust 接受（放行）20**（名单见 §5）。

### demos（1258 单元，check→test(sab)→build-exe→run 四步口径）
- 全步通过 **201**（basic 196 / rosetta 4 / workspace 1）；另 rosetta/79_metrics
  四步中 run rc=182 为其设计返回值（950 mod 256，与基线普查认定一致），
  计入则 202。
- 基线失败 6 名（F-1/F-2/F-3/F-4/F-5/F-8）在 Rust 侧全数仍败（无假绿）。
- 首败阶段分布：check 496 / test 556 / build-exe 5。
- 口径适配：cxu 的 run.sh 编排步未跑（只测编译器面 check/test/build-exe）；
  workspace 单元以成员 main 的 check/test 计。

## 3. 真缺口分类（按根因层级，非按 §12.8 旧名单）

headline：**§12.8 登记的五项残留全部证实存在，但都不是主要质量块**；
1095 跑真缺口的主体是两个未登记的「可调用面装配」缺陷——与 Zig 侧
1947 口径同一放大形态：少数根因 × 表面计数。

### R-1 check 形态：普通 fn 体内 std 关联面未装配（最大类）
- 签名：`Type Check Error: Undefined call: new / STR_PTR / PTR_BYTE_ADD /
  defer_ready / ready / push / from_millis …`，check 级独红 **670 文件**
  （tests 222 + demos 448）；run 级对应 chk-fail 428 跑。
- 可观察律（最小复现矩阵标定）：`Vec::new()` 等 std 关联调用与 std 宏调用
  **只在 @test 体内可解析**；普通 fn（含 main、helper）体内一律 Undefined。
  带类型注记与否、有无 @import 均不改变。demos 首败于 check 的 496 单元
  主体即此（main 里 `Vec::new()`/`HashMap::new()` 为 demos 通用写法）。

### R-2 test 形态：可调用面全域丢失
- 签名：check 通过、test 形态下 Undefined call —— UNDEF-STD 183 跑 +
  UNDEF-MODULE 91 跑（导入 .sla 模块函数如 `chain_point_sum`）。
- 最小复现：单文件仅含一个 @test 用 `Vec::new()` —— `sla check` 通过、
  `sla test` 双后端均报 `Undefined call: new`。
- 与 §12.8「CLI 前端 .sla 正文展开未接线」的分工：正文展开在 check 级
  只表现为 26 文件的 UNDEF-MODULE（登记项成立但量级小）；test 形态的
  丢失面远大于 check 级，是五期 p_tests 自包含夹具从未触及的盲区。

### R-3 parser 切片未覆盖（含 lang-gap 代差）
- run 级 PARSE 124 跑、check 级 65 文件（tests 44 + demos 21），另新增
  17 文件中 12 个同类。
- 已点名语法：`impl` 块（解析器自报 "impl declarations are not covered
  by the sla-parser slice yet"，如 demos/basic/12_impl_counter）、
  `@arena` / `arena {}`、lang-gap 新面（`-> T!` 返回糖出现在 sla_std/wrap
  声明里——导入即解析失败，如 test_unit_borrow_return_views；
  `&[T][owner]` 注记、fallible 载荷在 test 形态呈 TYPE-MISMATCH/PARSE）。
- 此类随 Zig 侧语言面演进会继续扩大（镜像时点差），属结构性代差。

### R-4 codegen 覆盖面（全部 check 通过、死在发射层）
- UNSUPPORTED-SAB 61 跑 + UNSUPPORTED-TEXT 53 跑（UnsupportedSabDirectFeature /
  UnsupportedFeature 响亮止）+ HOST 陷阱 21 跑（UndefinedMacro 7 = §12.8
  的 str_eq 臂；MemoryLeak 6；UseAfterMove 4；RegisterRedefinition 3；
  LLVMBackend 1；ForbiddenSyntax 1）+ CODEGEN-OTHER 9。
- §12.8 的 verified-encode 墙在 demos build-exe 首败仅 5 单元（被前两步
  大面积遮蔽，量级候 R-1/R-2 修复后重测）。

### R-5 checker 语义差（Rust 更严/不同，非装配问题）
- TYPE-MISMATCH 70 跑 / check 级 38 文件（借用可索引性、doc 参数借用形、
  match scrutinee 枚举性等）+ CHECKER-OTHER 26 跑 + USEAFTERMOVE 4 跑。
  逐名在 census-data/gaps_run_classed.txt 与 gaps_check.txt。

### R-6 负例放行（checker 诊断族缺失，20 文件全名单）
`neg_fallible_abi_payload`、`neg_fallible_bare_payload`、
`neg_fallible_result_discarded`（lang-gap A_min 诊断未实现）、
`test_error_deep_copy_map_field_copy`、`test_error_deep_copy_literal_live_binding`、
`test_error_fb011_switch_pattern_type`、`test_error_fb011_switch_undef_pattern`、
`test_error_def_triage_nested_use_after_move`、
`test_error_overload_generic_mix`、`test_error_overload_literal_ambiguous`、
`test_error_redeclaration_const_function`、`test_error_redeclaration_function`、
`test_error_redeclaration_macro`、`test_error_redeclaration_macro_param`、
`test_error_use_after_move_call`、
`test_error_view_direct_write_borrowed_field`、
`test_error_view_direct_write_borrowed_field_offset0`、
`test_error_view_local_same_field`、`test_error_view_local_same_field_offset`、
`test_error_view_root_reassign_while_borrowed`。
成族看：重声明检查（4）、视图写借用规则（5）、重载拒绝（2）、
deep-copy 字面量规则（2）、switch 模式（2）整族未实现。

### 布局敏感（不计独立缺口）
IMPORT 8 跑 / 4 文件：`tests/import_fixtures/pkgjson/…` 为源仓根相对
路径，镜像布局下以 corpus/ 为 cwd 可解析（解析后落入 R-2）；
Zig 侧以源仓根为 cwd 故不暴露。名单在 gaps 文件中（IMPORT 类）。

## 4. 双侧同红（存量，不归 Rust）
- run 级 16 跑：test_error.sla ×2（故意错误夹具）、reg1_dns_client_a ×2
  （外部 DNS 环境）、reg2d3_mutex_test_body sab、wrap_growvec_smoke sa、
  wrap 烟测族（http_client/net/process/regex/task）。
- check 级 144 名与 Zig 的 161 名交集（余 17 名 = Zig 红 Rust 绿，
  主体即 R-6 放行负例）。
- demos 基线 6 名全数仍败（§2）。

## 5. 实数读数建议（供主会话换算预估 80%）

分面实数（严格口径，与 Zig 侧同规矩：整文件/整单元全绿才算清）：
- tests run 级：**7 / 567**（1.2%）
- demos 单元：**201 / 1258**（16.0%；计 79_metrics 设计 rc 则 202）
- 负例：**131 / 151**（86.8%；带具名诊断 68 / 151 = 45.0%）
- （参考）check 级：1045 / 2022（51.7%）

**建议复合读数：已清 339 / 分母 1976 = 17.2%**
（已清 = tests 双绿 7 + demos 全绿 201 + 负例拒绝 131；
分母 = 567 + 1258 + 151）。

与预估的差距说明：五期验证面（自有 500 测 + fixtures）从未在普通 fn 体
与 test 形态下行使 std 关联面，属与 Zig 侧同型的结构性盲区
（语料按缺陷驱动、未按表面枚举——本普查即为补此分母而设）。
R-1 + R-2 两根若修复，run 级与 demos 的名集将成片翻绿；
建议修复后以同一脚本同口径复跑本普查，作为六期后续读数。

## 6. 数据附件（同仓 corpus/census-data/）
- `gaps_run_classed.txt`：1095 跑真缺口逐行（文件/后端/类/层）
- `gaps_check.txt`：check 级 Rust 独红 821 名
- `bothred_run.txt` / `bothred_check.txt`：双侧同红名单
- `neg_fail.txt` / `neg_accepted.txt`：负例明细与放行名单
- `demo_units.txt` / `demo_fail_units.txt`：demos 逐单元四步判读与失败名单
- `files_both_green.txt`：run 级双绿 7 文件
- `summary2.json`：本报告全部汇总数
