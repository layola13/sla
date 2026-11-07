# COORDINATION.md — 协调/集成线记录（Rust 版 sa_plugin_sla 差分实验）

本文件是全线共享的协调真源：crate 图定案、线位状态、裁决与纪律。
状态盘点时刻：2026-11-07 约 09:20 CST（各线仍在写，数字以盘点时刻为准）。
第二任协调线盘点更新：2026-11-07 约 09:35 CST（parser/checker/sab 三线
已交付，§2/§5/§6 已按交付与实证更新）。

## 1. Crate 图定案（六家裁断 Q1=A+、Q2=A，2026-11-07）

裁断全文：`~/workspace/consult-rust-split/VERDICT.md`。最终 10 成员，已落地根 `Cargo.toml`：

```
sla-abi            零依赖叶：plugin_api.zig 镜像（描述符类型/repr(C)/状态码）；不含导出锚点
sla-lexer          拥有 Span/Loc/TokenKind
sla-ast            → sla-lexer（方向锁，禁反向）
sla-parser         → sla-lexer, sla-ast
sla-checker        → sla-ast
sla-lowering       → sla-ast, sla-checker；内含 reachability 模块（plugin_reachability.zig 归此）
sla-codegen-text   → sla-lowering
sla-codegen-sab    → sla-lowering；五职责只做 crate 内模块族（format/registry/dispatch/layout/drop_cleanup/emit，职责一级轴 + std 域二级轴），一期不开 SAB 子 crate
sla-plugin         cdylib 顶层：导出锚点 + 管线接线；→ sla-abi + 全部管线 crate；无人依赖它
sla-oracle         dev-only 差分线束（对拍 + 边界转储）；→ 全部；无人依赖它
```

后端只能经 sla-lowering 的产出消费上游类型；禁止反向依赖与横向借类型边。
必改项落地状态：① sla-plugin 已在成员内、sla-abi 零依赖且无锚点 ✔；② reachability 已归置
（`crates/sla-lowering/src/reachability.rs`，协调线放的契约 stub：插入序 `ReachableSet`，
实现归 lowering 线；两后端必须消费同一集合，禁复制/跳过）；③ SAB 模块层拆分归 SAB 线按图执行；
④ HashMap 纪律见 §4；⑤ 边界转储归 sla-oracle（协调线工具形态先行，见 §5）；
⑥ ast→lexer 依赖边已加 ✔；⑦ 原 `sla-cli` 按定案图改名 `sla-oracle`（bin 名同步，
scripts/compare.sh、bench.sh 已改）。

## 2. 线位与交付状态（盘点时刻）

| 线 | crate | 状态 |
|---|---|---|
| A lexer/ast | sla-lexer, sla-ast | **已交付**：lexer.zig / ast.zig 全量镜像，场地 cargo test 绿（lexer 5 测试）；分歧登记 docs/divergences/lexer-ast.md（9 条） |
| parser | sla-parser | **已交付**（第二任协调线核盘）：13 文件、src 3,173 行（最大 decl.rs 487），cargo test 12/12（日志 docs/build-logs/parser-build-20261107.log）；分歧登记 docs/divergences/parser.md（D-P1..P13）已合编 |
| checker | sla-checker | **已交付**（核盘）：12 文件 3,648 行（最大 tests.rs 581），干净重编 cargo test 16/16（日志 docs/build-logs/checker-build-20261107.log）；分歧登记 docs/divergences/checker.md（D-C1..D-C10）已合编；D-P5 侧表警告已由本线 BindingId 身份键控自带处理 |
| lowering + 文本 codegen | sla-lowering, sla-codegen-text | lowering 本体已落盘成形（真实 API：CheckInfo/LoweringError/node_key，协调线占位 Lowering 已被取代）；codegen-text 12 模块已落 10，余 call.rs / println.rs 在写——**集成轮全量构建的当前唯一门** |
| SAB codegen | sla-codegen-sab | **已交付**：拆分图 docs/sab-decomposition.md（20,024 行 → 50 模块/9 组）；首切片同模型对拍实证（76B sha 000fb0e6…、427B sha 31fc5e86… 与 sci 现头原编码器探针 cmp 全同）；cargo 10/10；分歧 16 条 docs/divergences/sab.md 已合编 |
| 协调线 | sla-abi, sla-plugin, sla-oracle, 根接线 | **本轮已补自有欠账**：sla-abi 类型面全量镜像完成（descriptor/context/broker/handler 四模块，布局按 abi_probe 实测钉死，cargo test 5/5）；sla-plugin 描述符双锚点已接线（skills 静态表逐字镜像，handle_command 为响亮 Failed 桩、第二 ABI 未导出且已声明）；sla-oracle 已实现到可跑（D-P3 seed 预扫描 + 六段边界转储 + 分阶段计时），待 workspace 全量编译放行后冒烟；接线修复两件：sla-ast 根再出补齐全类型面、sab generate_from_program 桩签名改接 lowering 真实 API（见 §6） |

全 workspace 场地构建：round2（2026-11-07 上午，Kaggle 2201）10 crate 全部编译通过。
行数纪律：`check_file_lines.sh` 通过——35 个 .rs 全部 ≤700（最大 lexer.rs 576）。

## 3. 裁决 D-A1（AST 所有权形态，协调线裁决，2026-11-07）

**默认 owned 树（Box/Vec/String）；禁止各线私用 Rc 绕行。** 若某线发现 Zig 侧确有
共享/别名节点需求，必须停下、登记 Zig 源码位置，并提出 ONE 个 workspace 级
arena/index 方案送协调线裁决，不得先斩后奏。与六家拆分裁断无冲突（裁断已确认按此执行）；
若后续裁断类决定与之冲突，以六家裁断为准并在此注明覆盖。

与边界转储的张力（记入 REPORT）：owned 树使 AST 转储天然是值拷贝、无别名歧义，
对差分归因是净利好；张力仅在性能面（深拷贝成本），一期不处理。

## 4. 全线纪律（即刻生效）

1. **no improvements**：禁止移植时顺手修正/美化 Zig 的怪癖——怪癖照原样镜像并登记分歧。
   唯一例外是已裁决的有意分歧（如 D-L2 的 Rust 钳制，见 §5），例外必须登记在案。
2. **分歧三分类**：每处分歧必须归类为「Zig 侧缺陷候选 / 本移植缺陷 / 有意为之（规约含糊归入
   有意为之并注明）」，给依据；不许为了对拍绿悄悄改 Rust 迁就 Zig。
3. **HashMap 迭代序禁令**：std HashMap/HashSet 的迭代顺序不得进入任何发射路径；
   发射序一律由插入序结构（Vec 索引表 / BTreeMap / arena 序）决定。Rust 无 lint 可强制，
   靠评审 + oracle 转储确定性校验执行（根 Cargo.toml 已注）。现存两处 HashMap 使用均为
   查表型、暂不违规，登记在案请两线自查保持：checker `scope.rs` 的 `StateSnapshot`
   （状态快照，若日后按其迭代产出诊断顺序即违规）、sab `pool.rs` 的 `index`
   （字符串池索引，发射序必须由插入序 Vec 决定）。
4. **700 行/文件**：逼近上限先拆后写；check_file_lines.sh 覆盖全 workspace。
5. 接线归口：根 Cargo.toml 与各 crate lib.rs 的接线只由协调线改；跨 crate 公开接口
   在各线交付齐前不定版。
6. **反馈环（用户 2026-11-07 常设政策）**：分歧定案为「Zig 侧缺陷候选」者，
   当轮在 docs/feedback/ 落反馈单（最小输入/机制/Zig 行号/建议形态），
   不攒到阶段终报；主会话据此派 Zig 侧修复线与移植并行。Rust 侧仍只
   镜像 + 登记，永不替 Zig 改行为。已发：FB-001（D-CK1)、FB-002
   （DEF-LEX-ESCAPE-EOF），均附实证。

## 5. 对拍口径与已知有意分歧

- 期望输出：fixtures/expected/*.sa（场地 Zig 插件 `sa sla build` 产出，sha 已核，见
  fixtures/PROVENANCE.md）。切片端到端对拍**尚未跑通**：管线未集成（parser/checker/
  lowering/codegen 未交付齐），sla-oracle 仍为骨架——不许把任何中间状态说成对拍结果。
- **已知有意分歧 D-L2（=K-1）**：Zig lexer 未闭合字符串尾 `\` 时 index 越界
  （loc.end 可达 buffer.len+1），Rust 钳制到 buffer.len，**不迁就**。主会话已转主线缺陷候选。
  **协调线已实证（2026-11-07）**：原 lexer 探针五例，尾单反斜杠形态
  loc.end == len+1（字符串与模板两路），对照组恰为 len；模板路另有
  越界后不产 eof、原地重复 invalid 的附带现象。证据 docs/probes/dl2_probe.out.txt，
  反馈单 FB-002。
- **SAB 一期对拍口径（SAB 线交付声明、协调线采纳）**：同模型对同模型
  （编码器 + emit 主干对 fixture01 手工模型，与 sci 现头探针 cmp 全同）；
  fixture01 端到端（println/str_eq 的 std 表面展开）属二期，**不列一期判据**；
  SAB-D13（emitBranch operands[2] 重复 then 标签）是字节级原样复现点，
  oracle 逐函数字节比对时勿误判为本端口缺陷。
- 未解对拍风险（交 codegen 线，详见 docs/DIVERGENCES.md 的 D-C4/D-C5）：期望输出的
  @import 列表不是源导入的纯函数；全局 tmp/label 计数器存在不可见消耗点
  （07 中 tmp_7 被吞、未发射 MERGE 也占号）——找不到确切分配点即记本移植缺陷。

## 6. 在途问题登记

- **SAB 线 sleb128 单测红 — 已结案销账（2026-11-07）**：裁决 = **本移植缺陷
  （测试向量笔误）**。原向量 `[179, 159, 127]` 是 -12365 的编码、被误作
  -12345 的期望；实现产出 `[199, 159, 127]` 与 Zig 原编码器逐字节一致。
  证据：协调线场地探针（原 sci encodeSleb128 对 -12345 出 `c7 9f 7f`、
  对 -12365 出 `b3 9f 7f`，docs/probes/sleb_probe.out.txt）与 SAB 线自查
  结论一致。程序注记：向量在失败后约 31 秒被改、早于书面裁决（mtime
  取证，见 DIVERGENCES 合编结案节）——方向经独立证据证明正确、实现
  从未被迁就修改，仅记程序瑕疵。
- **checker D-C1（=D-CK1）全管线复现 — 已确认（2026-11-07）**：协调线以
  最小输入跑原 Zig 插件（场地树 699f13d4）：floor 与无遮蔽 control 均报
  UseAfterMove rc=1，遮蔽件 **rc=0 漏检**，与名字键控快照机制预测吻合。
  结论已写入 DIVERGENCES 合编并回主会话；反馈单 FB-001。
- **D-P5（parser→checker 侧表警告）— 已销账**：checker 线以 BindingId
  身份键控侧表自带处理，接线知悉即可，无协调线动作。
- **D-P3 seed 责任 — 已接**：sla-oracle 已实现协调方预扫描（.sla 导入的
  struct/enum 名经 seed_known_type_names 注入，失败响亮记录不静默吞，
  对照 D-P11）；现 fixtures 仅导入 .sai/.sa，seed 为空集属构造性结果，
  预扫描通道本身随 fixture 集扩 .sla 导入时即生效。
- **接线修复两件（协调线，2026-11-07）**：① sla-ast 根再出补齐全部
  pub 类型（lowering 线按根路径消费 FuncDecl/Primitive/Param 等，
  原再出仅 Program/Node/Type，workspace 构建因此中断）；
  ② sla-codegen-sab 的 generate_from_program 桩签名由已消失的协调线
  占位 `sla_lowering::Lowering` 改接 lowering 真实面（&Node +
  &dyn CheckInfo，与 text 线同形），函数体响亮错误不变。
- round2 测试格局（日志原文口径）：abi/ast/checker 各 0 测试 ok；sab 6 passed / 1 failed；
  **cargo test 在 sab 失败后 fail-fast，其余 crate（lexer/parser/lowering/codegen-text/
  plugin/oracle）该轮未运行**——round1 已证 lexer 5 测试绿。集成轮跑全量必须加
  `--no-fail-fast`，否则红点会遮住后续格局。
- round3 格局（2026-11-07，--no-fail-fast，日志 workspace-test-20261107-round3.log）：
  abi 一度 4 红（协调线镜像误用 Option<裸指针>，已改裸指针后 5/5 全绿）；
  lexer 5 / parser 12 / checker 16 / lowering 与 sab 各套件全绿；
  codegen-text 在写（余 2 模块）致 workspace 整体未合拢，plugin/oracle
  构建候其落盘。
- **集成门（2026-11-07 最新）**：codegen-text 20 个编译错误已由该线自修
  清零——**workspace 全量构建通过（10/10 crate）**，round4 全量测试
  `--no-fail-fast` 全绿（59 passed / 0 failed，日志
  workspace-test-20261107-round4.log），plugin .so 双锚点 nm -D 验活
  （D saasm_plugin_descriptor_v1 / T …_fn），oracle 已接通 generate
  （checker 侧表 → CheckInfo 适配，协调线接线）。**对拍现卡两处**
  （详见 DIVERGENCES 集成发现）：INT-1 checker 对在场 return 误报
  missing return（3 行最小复现，归 checker 线）；INT-2 合同加载/
  导入展开段 crate 图无主（println 等 std 调用无从注册，二期立项）。
  INT-1 清零后无 std 依赖程序先行逐字节对拍；计时随后。

- **子系统线 D（lowering+text）交付收讫（2026-11-07，四线全齐）**：
  分歧登记 divergences/lowering-text.md（D-T1..D-T14）合编入
  DIVERGENCES.md；D-C4/D-C5 经其定层**双结案**（协调线先前的
  test 可达性假设证伪、已更正）。候复验命令
  `cargo test -p sla-codegen-text --test fixture_parity` 已在场地三
  跑定：**5/5 fixture 逐字节全等**。
- **接线裁定三件（协调线，2026-11-07）**：① sla-lowering 的未用
  sla-checker 路径依赖**删除**——CheckInfo trait 边界已取代之，
  保留只会误记 crate 图方向且未来成环（Cargo.toml 内有注记）；
  ② reachability.rs 桩**归 sla-lowering 所有**，一期保持契约桩、
  实现列二期（text 线未消费真实可达集即完成对拍）；③ CheckInfo
  地址键契约（D-T10）对一切接线方生效：喂入树不得 clone/搬动；
  @test 体清空（D-T7）属流水线职责，对拍台/oracle 以空体喂入
  匹配 build 模式 golden。
- **反馈环第二批（FB-003..FB-009）**：lowering/text 线 (c) 类七条
  （D-T9 genStmt 静默吞语句最重、D-T3 sum 硬编码、D-T4 未知被调用
  名假定 EXPAND、D-T5 println 1 字节、D-T6 长度口径、D-T7 @test
  流水线裁剪、D-T8 死分支）已按常设政策落单，随本轮报告回主会话。
- **round5 格局（2026-11-07，场地三，清单校验过的树）**：
  `cargo test --workspace --no-fail-fast` = **64 passed / 0 failed**
  （round4 为 59；增量含 fixture_parity 5/5 定谳）。D-CK1 另以
  现头 sla 6002b386 在场地三复证：floor/control rc=1、shadow
  rc=0，与 699f13d4 口径一致，FB-001 证据升级为现头口径。
- **同步协议硬化（2026-11-07 事故后）**：场地三预置旧快照与覆盖式
  解压交叠，一度留下 3 个旧版/缺失文件（println.rs、
  fixture_parity.rs、lowering-text.md）而文件计数仍相同——
  sync_to_venue.sh 已改三跳形态（擦除同步路径 → 已证形态解压 →
  md5 清单全量校验，BAD≠0 即红停）。教训与主线「双确认」同源：
  计数对上不等于内容对上。
- **二期收编（2026-11-07，三期协调线）**：开工前全量同步 + 独立
  复核（场地清单 150 文件、非 OK 数 0）+ 基线复现 64/0 后开线。
  线组与交付：checker-INT1 线（INT-1 销账，分类 (a) 本移植缺陷，
  checker 24/24）→ checker 扩围线（清单逐项攻下、80/80，剩余
  3 项登记在 checker.md）与 oracle 线（全管线对拍达成：e2e_b/
  e2e_c 逐字节全等、e2e_a 仅剩 D-O1；计时同窗补测毕）并行；
  SAB 线（B2/B3/B4 落地、generate_from_program 真实驱动、17/17）；
  收尾两件：SAB-D17 双轨镜像（裁决 = 逐字镜像，真产物字节级
  验证，FB-015 气味面保留）与 D-O2 接线销账（build 模式 @test
  清空归 oracle 流水线，prune 原位 body.clear() 守 D-T10 地址键
  契约，build 为默认模式、--no-build-mode 与旧基线逐字节同 sha）。
  二期场地同步纪律：各线只同步自家 crate/文档路径（逐路径擦除 →
  tar 管道 → md5 双确认），全量 sync_to_venue.sh 只归协调线在
  开工/收编两点运行——二期全程无同步事故（D17 线中途一次场地
  服务重启，以清单复核确认落地完整，未盲目重跑）。
  合编后：分歧 82 条在册、反馈环累计 18 单（FB-001..FB-017 +
  FB-020）、workspace 137/0、100 个 .rs 全 ≤700。拆分图 B 组
  状态已更新（B2/B3 ✅、B4 ◐ 本段范围 ✅）。三期候选序见
  REPORT §9.4。

## 7. 场地连续性

- 现场地：Kaggle 场地三（kernel icbnsk/muse-kaggle-venue-vpsdirect3），
  hop 入口 `bash ~/workspace/kaggle-venue-tunnel/hop_vpsdirect.sh 2202`，
  hold 至 2026-11-07 约 19:43 CST（12h 硬顶约 20:43）；场地内为现头
  （sci e82d2865、sla 6002b386、sa 0.1.8、INSTALL OK）+ Rust 1.99.1。
  同步脚本默认端口已切 2202。场地二（2201）已到期弃守。
- 后继场地（VPS 直连形态、新回环端口、预装 Rust）由场地线预建，入口候主会话发放。
- 热切换规程：切换前先把本地主本（唯一真源）git 快照；新场地到后先同步
  （scripts/sync_to_venue.sh 改端口参数，含清单校验）跑一轮 `cargo test --workspace` 复现当前
  红/绿格局，再继续各线工作。**场地死亡不回落本机构建**（用户定规），在 REPORT 登记候新场地。

## 8. 三期开工（2026-11-07，第三任协调线）

开工核查（本线执行）：本地主本 = 快照 `1b0518c`、工作树干净；场地三（2202）
存活（hostname 66cf9c74eaf1、sa 0.1.8、sla 树在位）；全量同步后清单校验经
场地侧人工复核 **0 差异**（注记：sync_to_venue.sh 的校验步在 BAD=0 时因
`grep -cv` 退出码 1 与 `set -e` 相撞假失败、未打印 SYNC_VERIFY_FAILED，
树本身经 `md5sum -c` 直查 463 文件全 OK——脚本瑕疵登记在案，各线同步后
一律以 md5sum -c 直查为准）；基线复现 `cargo test --workspace
--no-fail-fast` = **137 passed / 0 failed**，与二期收编口径一致后开线。

### 裁决 D-A2（INT-2 合同段归属，协调线裁决，2026-11-07）

六家 A+ 图 10 成员不动、不新增依赖边，合同段按 Zig 原拓扑归位：

- **.sai 合同解析 → sla-parser 新模块族**（镜像 contract_parser.zig，
  产出 ast 层合同声明）。sla-parser 禁依赖 sla-checker（方向锁不变），
  合同声明以 ast 形态流动。
- **导入展开编排 → sla-plugin 接线层**（镜像 plugin_import_expand.zig 的
  loadImportedContracts 切片：路径解析、合同加载、prescan 种子、注册表
  填充）；sla-oracle 经同一接线消费，**禁止 oracle 私设第二套展开**。
- **注册动作**经 sla-checker 既有 `register_extern_func`（checker.rs:294）；
  INT-2 线不改 checker 源码，注册面不足即停下登记、候协调线定序。
- SAB D 组表面（sab_codegen 内的 surface_load 等）仍归 sla-codegen-sab
  按拆分图 D 组自理，与本裁决的合同段是两套不同机器（合同 = .sai →
  extern 注册；表面 = SAB 规则文件 → lowering 规则），勿混。

### 三期线位（并行，路径互斥）

| 线 | 辖区（只许动这些路径） | 任务 |
|---|---|---|
| INT-2 | crates/sla-parser/**、crates/sla-plugin/**、crates/sla-oracle/src/** | 合同段立项：5/5 fixtures 过 check 段并逐件销账（println 族 + 01 的 str_eq 第二挡），全管线对拍推进 |
| SAB-C | crates/sla-codegen-sab/** | 拆分图 C 组（prescan_consts/prescan_bindings/symbols）；INT-2 解锁后接 D 组 |
| CHK3 | crates/sla-checker/** | 剩余 3 项：关联调用接收者推导全形态、match 守卫绑定可见性/消费、plain-if 条件收 integer |
| HND | 只读 + docs/phase3/handler-eval.md | handler 活体对拍（第二 ABI 锚点）接线评估，先评估后定是否纳入三期 |
| ORC | scripts/**、results/p3/**、docs/phase3/oracle-timing.md | 计时同口径协议 + Zig 基线先行；Rust 侧测量候 INT-2 落地后启动（协调线通知） |

接线口径（三期操作注记，承 §4.5）：各线可改**自家 crate 的 lib.rs** 增删
mod 声明（交付注记中逐条列明）；**Cargo.toml 依赖边仍只归协调线**，
新依赖边需求停下登记。反馈单号段：INT-2 = FB-021..024、SAB-C =
FB-025..028、CHK3 = FB-029..032、ORC = FB-033..034、HND = FB-035；
号段用尽或越界先报协调线。各线不 git commit，协调线在集成点统一快照。

## 9. 三期收编（2026-11-07，第三任协调线）

线组七条全交付：INT-2（合同段 + A0 切片）、SAB-C（C 组）、CHK3
（checker 余 3 项）、HND（评估）、ORC 阶段 A（协议 + Zig 基线）、
INT21（名字特判臂，INT-2 跨线移交）、SAB-D（D 组，INT-2 解锁后
开线）；ORC 阶段 B（同口径对比）随收编启动、结果入 REPORT §10。

- **集成终局**：协调线全量同步（md5 清单直查 0 差异）后场地
  `cargo test --workspace --no-fail-fast` = **294 passed / 0
  failed**（二期基线 137 → 三期净 +157）；129 个 .rs 全 ≤700 行。
- **集成期协调线自办四件**：① 过期 pin 更新（sla-plugin
  contracts_pipeline 的 println 阻断 pin 改为关闭态，INT-2 自
  登记的预定动作）；② driver_e2e.rs 拆分（765 → 600 + 新
  driver_e2e2.rs 362，测试名与计数 11 不变，纯行数纪律）；
  ③ ABI_MAP §1 更正注（FB-035）；④ HND 四候选收编落单
  FB-035..FB-038。
- **集成发现（cargo 陈旧二进制，登记防复发）**：擦除重解压同步
  后，源文件 mtime（tar 保留的本地时刻）可能旧于场地既有构建
  指纹，cargo 静默复用旧测试二进制——本次 pin 更新险被误判为
  红（实跑旧测试名）。规程：全量同步后、跑门前先
  `find crates -name '*.rs' -exec touch {} +`，或以输出特征
  （测试名/版本串）自证二进制新鲜度。此与一期同步事故同源：
  同步的「内容真」之外还要「构建真」。
- **裁决三件（协调线，登记候四期/主线）**：① INT2-2
  （ExternSig.return_fallible）不阻塞、原子落地条件已登记，
  三期不做跨 crate 手术；② oracle 内 5 行登记垫片（oracle→
  plugin 边）与 ③ SAB 烘焙目录 build.rs 注入（现经
  SLA_SAB_BAKED_SA_STD_DIR 环境槽）同为接线清理项，非阻塞、
  候四期一并处理；面 B（compile_handler）候 FB-035 主线裁定。
- **三期总账**：分歧在册 **106 条**（82 + 24）；反馈环累计
  **24 单**（三期新发 FB-025/FB-026/FB-035..FB-038 六单）；
  对拍面：fixtures 5/5 全管线（3 件逐字节全等 + 2 件仅 Zig
  哈希序差）、e2e_b/e2e_c 维持全等、A0 命令矩阵 33/33 活体
  全等、SAB 无表面程序发射中性实证。剩余响亮未接线：SAB
  E–I 组、导入宏/workspace 包/@expand_tuple、`.sla` 正文展开、
  描述符管线命令、关联块前裸名段（D-C22）。

### 计时证据口径（用户 2026-11-07 10:11 裁决，三期完工交付物）

全部修复后 Zig 版打 tag，等 Rust 分支完成做速度对比：Rust 全面超越或
超 10% 即召六家裁决 Rust 替代 Zig。本线计时自此是该裁决的**正式证据**：
三期完工报告必须同批交付同口径对比报告（同场地、同语料集、冷/暖各多轮
取中位数；编译速度 = 端到端墙钟含启动段拆分；运行速度口径如实定义——
产物逐字节全等处运行速度由构造相等、逐件列明，不全等件排除出运行
结论）。**Rust 侧缺段（如合同段未补齐前）必须明说「尚非同口径」并给
缺段清单**；二期 3–4 ms vs 26 ms 只登记为初步观测、不作结论引用。

### 口径变更注记（2026-11-07 14:37 用户裁定，supersede 上段）

10:11 的「速度对比 → 六家裁决是否替代」门已作废：替代由用户直接决定（Zig 完整修复打 tag 后封存为参照实现，此后主线走 sla 独立版本）。上段计时证据口径自此仅作历史；四期计时为记录项（见 REPORT.md §11.6），不作门、不触发裁决。

## 四期状态（2026-11-07 收编，四期协调线）

- **完工**：五项范围全合入本地主本——SAB F 组全量落地 + E/G/H/I 四组并行落地、导入展开面（INT2-4/5/7 关闭）、D-C22 销账、新顶层 crate **sla-cli**（独立二进制 `sla`，命令全集接通）、ORCP 决定性双侧计时补跑入档。
- **总账**：workspace **483 passed / 0 failed**（三期 294 → +189；分 crate 见 REPORT.md §11.2）；188 个 .rs 全 ≤700；fixtures 5/5（3 逐字节 + 2 仅 D-C23）；分歧在册 **150**（106 + 44，销账 D-C22/SAB-D26/INT2-4/5/7）；反馈累计 **44 单**（24 + 20：FB-039..041、045..048、053、059..070，全部候主会话确认）。
- **死亡点**：默认形态 = driver step 2 → `flattenStdSnippet` 边界（SAB-D31，sci 宿主面，trait + 响亮默认）；替身注入后 = gen_call println 臂（D 组）。
- **设计约束在册**：SAB 注册表联邦化（从声明生成、可按包联邦）为本仓设计约束，落实点与差距见 §11.8；sla_std 薄包为主流写法的现状与两项差距见 §11.9。
- **场地**：盒为主战场（plhome-rust4 隔离）、Kaggle 2203 为 CLI 线场地（2201 探活无会话已登记）；争用无、点名权未触发。
- 细节全在 REPORT.md §11；四期快照 = 本次提交，推送归主会话。
