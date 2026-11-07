# COORDINATION.md — 协调/集成线记录（Rust 版 sa_plugin_sla 差分实验）

本文件是全线共享的协调真源：crate 图定案、线位状态、裁决与纪律。
状态盘点时刻：2026-11-07 约 09:20 CST（各线仍在写，数字以盘点时刻为准）。

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
| parser | sla-parser | 在途：parser.rs 约 480 行 + error/numlit/token 模块已落盘 |
| checker | sla-checker | 在途：checker.rs 约 374 行 + diag/rules/scope/types_util 已落盘 |
| lowering + 文本 codegen | sla-lowering, sla-codegen-text | lowering 仍为骨架 + 协调线 reachability 契约 stub；codegen-text 仍为骨架 |
| SAB codegen | sla-codegen-sab | 在途：ir/emit/encode/pool/literals/leb128 已落盘；单测 6 过 **1 红**（见 §6） |
| 协调线 | sla-abi, sla-plugin, sla-oracle, 根接线 | abi 仍为 3 行骨架（类型镜像待补，材料在 docs/ABI_MAP.md）；plugin 仅 PLUGIN_NAME，锚点待 abi 类型完备后接线；oracle 为骨架（职责契约已写入 main.rs 头注） |

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

## 5. 对拍口径与已知有意分歧

- 期望输出：fixtures/expected/*.sa（场地 Zig 插件 `sa sla build` 产出，sha 已核，见
  fixtures/PROVENANCE.md）。切片端到端对拍**尚未跑通**：管线未集成（parser/checker/
  lowering/codegen 未交付齐），sla-oracle 仍为骨架——不许把任何中间状态说成对拍结果。
- **已知有意分歧 D-L2（=K-1）**：Zig lexer 未闭合字符串尾 `\` 时 index 越界
  （loc.end 可达 buffer.len+1），Rust 钳制到 buffer.len，**不迁就**。主会话已转主线缺陷候选。
- 未解对拍风险（交 codegen 线，详见 docs/DIVERGENCES.md 的 D-C4/D-C5）：期望输出的
  @import 列表不是源导入的纯函数；全局 tmp/label 计数器存在不可见消耗点
  （07 中 tmp_7 被吞、未发射 MERGE 也占号）——找不到确切分配点即记本移植缺陷。

## 6. 在途问题登记

- **SAB 线单测红 1**：`leb128::tests::sleb_known_vectors` FAILED（场地 round2 实测，
  日志 docs/build-logs/workspace-test-20261107-round2.log）：断言 left `[199, 159, 127]`
  vs right `[179, 159, 127]`（leb128.rs:102），首字节差 24。是实现错还是测试向量错，
  归 SAB 线裁决并在本文件登记结论；裁决前不许改测试迁就实现（no improvements 的镜像纪律）。
- round2 测试格局（日志原文口径）：abi/ast/checker 各 0 测试 ok；sab 6 passed / 1 failed；
  **cargo test 在 sab 失败后 fail-fast，其余 crate（lexer/parser/lowering/codegen-text/
  plugin/oracle）该轮未运行**——round1 已证 lexer 5 测试绿。集成轮跑全量必须加
  `--no-fail-fast`，否则红点会遮住后续格局。

## 7. 场地连续性

- 现场地：Kaggle 场地二，hop 入口 `bash ~/workspace/kaggle-venue-tunnel/hop_vpsdirect.sh 2201`，
  名义 hold 至 2026-11-07 约 09:15；09:20 盘点时仍存活（已超时，随时可能断）。
- 后继场地（VPS 直连形态、新回环端口、预装 Rust）由场地线预建，入口候主会话发放。
- 热切换规程：切换前先把本地主本（唯一真源）git 快照；新场地到后先同步
  （scripts/sync_to_venue.sh 改端口参数）跑一轮 `cargo test --workspace` 复现当前
  红/绿格局，再继续各线工作。**场地死亡不回落本机构建**（用户定规），在 REPORT 登记候新场地。
