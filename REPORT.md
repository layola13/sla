# REPORT.md — Rust 版 sa_plugin_sla 差分实验 · 一期报告

状态时刻：2026-11-07 约 09:25 CST。本报告是**阶段性**的：各子系统线仍在推进，
未完成项逐项明示，不把中间状态说成验收结果。本地仓仅本地 git，不建远端、不推送。

## 0. 本实验的目的（用户定调）

不是「要不要迁 Rust」，而是**差分**：用一套独立的 Rust 第二实现当参照系，
照出 Zig 实现自身发现不了的底层 bug。分歧是成果、不是失败；每处分歧先登记、
再分类（Zig 侧缺陷候选 / 本移植缺陷 / 有意为之），禁止把 Rust 侧悄悄改成迁就 Zig。

## 1. 一期交付物与状态

| 交付 | 状态 |
|---|---|
| ABI 摸底 docs/ABI_MAP.md | **完成**：双描述符符号（数据 + fn）、PluginDescriptor 64B 逐字段、宿主加载/校验/命令分发路径、Context 约 88B 手工镜像面、handler ABI（state field 40B / options 16B / result 48B、返回码语义、result_free 所有权）、命令面全表 |
| 分歧登记 docs/DIVERGENCES.md + docs/divergences/&lt;lane&gt;.md | **已立并在录**：协调线 5 条（D-C1..C5）+ lexer/ast 线 9 条（D-L1..L6、D-A1..A3）；checker/sab 线随交付补入 |
| Crate 图定案 | **完成**：六家裁断 Q1=A+ / Q2=A（全文 ~/workspace/consult-rust-split/VERDICT.md），10 成员已落根 Cargo.toml，见 docs/COORDINATION.md §1 |
| 垂直切片对拍（3–5 程序逐字节） | **未跑通**：期望输出已备齐（fixtures/expected/*.sa，Zig 侧产出、sha 已核，见 fixtures/PROVENANCE.md），但 Rust 管线未集成（parser/checker 在途、lowering/codegen-text 未交付、sla-oracle 仍为骨架），字节对拍尚无结果——这是本报告最大的未完成项 |
| 分阶段计时对比 | **半成**：Zig 侧基线已实测（§5）；Rust 侧因管线未集成未测 |
| 700 行纪律 | **达标（盘点时刻）**：check_file_lines.sh 通过，35 个 .rs 全 ≤700（最大 lexer.rs 576） |
| 全 workspace 构建 | 场地两轮实测：10 crate 全部编译通过；测试格局见 §4 |

## 2. 这条差分线已经照出了什么

按价值排序（详情在 docs/DIVERGENCES.md）：

1. **K-1（=D-L2，Zig 侧缺陷候选，主会话已转主线登记）**：lexer 字符串/模板转义扫描在
   「未闭合字符串以 `\` 结尾」时 index 可推进到 `buffer.len + 1`，产出 `loc.end` 越界的
   token；下游按 loc 切片在安全构建下 panic、ReleaseFast 下 UB。Rust 借用/切片纪律
   **逼**出了这个点——Zig 侧自查很难看见，因为正常输入永不触发。Rust 钳制行为已裁决
   不迁就，列为已知有意分歧。
2. **D-C2（静默兜底家族）**：plugin_handler.zig 的 `abiSlice` 对 `len == 0` 无条件返回
   空切片，null/野指针不查；`fields_len == 0` + null fields_ptr 被静默接受。与主线已知的
   `else |_| <字面量>` 吞错同族——差分线给这类点提供了**成建制的发现机制**：
   每一个 Zig 的无检查切片/兜底，在 Rust 镜像时都会被迫表态。
3. **D-C3**：std 导入解析的兜底链末端硬编码了个人机器路径（`/home/vscode/projects/...`），
   环境相关行为被写死进发布代码；Rust 侧只保留 env/祖先根解析（有意分歧候选，待裁决）。
4. **D-C4 / D-C5（对拍驱动出的行为学发现）**：期望输出的 @import 列表不是源导入的纯函数
   （疑似剪枝前的 @test 体引用也计入展开）；全局 tmp/label 计数器有**不可见消耗点**
   （未发射的 MERGE 标签也占号、07 中 tmp_7 被无声吞掉）。这类「输出不可由局部规则
   复现」的点正是字节对拍的核心猎物：集成轮若 codegen 找不到确切分配点，即立移植缺陷
   并反推 Zig 行为。
5. **SAB 线在途分歧（未裁决）**：sleb128 已知向量单测红——Rust 实现产出 `[199,159,127]`、
   测试期望 `[179,159,127]`。是移植实现错还是向量本身照抄了 Zig 的某个怪癖，
   正由 SAB 线按 no-improvements 纪律裁决中；无论哪侧错，这都是差分在起作用。

## 3. 还能往哪照（差分价值排序，供二期决策）

1. **SAB codegen（最高）**：20,024 行 Zig、互递归五职责、二进制格式——字节级对拍 +
   边界转储（逐函数双后端字节）能把分歧定位到单个函数/单个 section，是全仓差分密度
   最高的子系统；且 SAB 是生产路径（SA 文本只是教学/调试镜像），照这里等于照生产面。
2. **type_checker / 所有权检查器**：语义判断类分歧（同一程序一侧拒一侧放）不像字节
   分歧那样自明，必须靠语料级差分（正例集 + 负例集双向跑）才能照出；本期 checker 线的
   scope 建模已在暴露 Zig 的扁平 StringHashMap 全局命名空间等结构性事实。
3. **插件 handler / 命令面**：ABI 面小而硬，宿主加载 + skills/check/build 命令可用真宿主
   做活体对拍（Rust cdylib 挂进真 `sa` 跑），成本低、结论硬，适合做集成轮第一站。
4. lexer/parser 差分价值已在兑现（K-1），后续以语料扫为主、边际递减。

## 4. 构建与测试实测（Kaggle 场地二，同机同窗）

- round1（强制 touch 全量重编）：10 crate 编译通过，lexer 5 测试全绿。
- round2（定案接线后）：编译通过；测试 abi/ast/checker 各 0 测试 ok，sab 6 过 1 红
  （sleb 向量，§2.5），cargo test fail-fast 使其余 crate 该轮未跑。日志：
  docs/build-logs/workspace-test-20261107-round{1,2}.log。
- 集成轮注意：全量测试须 `cargo test --workspace --no-fail-fast`，否则首红遮蔽全格局。

## 5. 性能节（次要；只报实测，口径：场地 Kaggle 同机、热缓存、切片小文件 287–729B）

- Zig 原插件端到端 `sa sla build`，每文件 7 轮取中位：01–07 五个切片均约 **30 ms**
  ——此规模下宿主进程启动占绝对主导，编译本体在 SLA_PROFILE 分阶段口径下为
  sub-ms 至数 ms 级；唯一例外是 01 首次触达的 contract load 81 ms（缓存后 0 ms）。
- Rust 侧：**未测**（管线未集成，无可测的端到端产物）。主会话对用户的预估
  「产物逐字节一致故下游性能 0 变化；编译速度忠实移植大概率持平 ±20%」——
  前半句是逻辑推论（字节一致 ⇒ 下游无差），后半句的 ±20% **本期既未证实也未证伪**，
  待集成轮用 scripts/bench.sh 同口径补测。在此之前不引用该数字作结论。

## 6. 移植规模参照（Zig 行数 → Rust 现况）

- Zig 参照总量：src 39 文件 83,206 行（含 plugin_tests 9,544；实现约 73,662）。
  大文件：sab_codegen 20,024 / codegen 16,288 / lowering_rules 9,705 /
  type_checker 6,098 / parser 4,478。
- Rust 现况（盘点时刻、仍在增长）：35 个 .rs、约 5,188 行。其中 lexer+ast 两 crate
  为全量镜像已交付（lexer 576 行对 lexer.zig、ast 五文件对 ast.zig），其余为在途或骨架。
- 拆分图：sab_codegen 的 20k 行按裁断 Q2=A 落为**单 crate 内模块族**
  （format/ registry/ dispatch/ layout/ drop_cleanup/ emit/，职责一级轴 + std 域二级轴，
  registry 按 std 域分文件卡 700 行）——互递归事实决定拆成子 crate 必须引入 Zig 中
  不存在的 trait 层、反而损害差分对位，故停在模块层（裁断依据见 VERDICT.md）。
  codegen.zig / lowering_rules.zig 同理只做 crate 内模块拆分。

## 7. 未完成项与场地状态（如实登记）

1. 垂直切片字节对拍未跑通（管线未集成）——待 parser/checker/lowering/codegen-text
   四线交付后由协调线在集成轮完成：workspace 构建 → scripts/compare.sh → scripts/bench.sh。
2. sla-abi 类型全量镜像未写（现为 3 行骨架，材料齐备于 docs/ABI_MAP.md）；
   sla-plugin 导出锚点待 abi 类型完备后接线——归协调线下一轮。
3. 边界转储（裁断必改项 5）仅在 sla-oracle 头注里立了契约，未实现。
4. 场地：Kaggle 场地二（hop 2201）名义 hold 至约 09:15，盘点时仍存活但随时可断；
   后继场地（VPS 直连新端口）候主会话发放。场地一断，本线按规程停手登记、
   **不回落本机构建**。本机主本为唯一真源，已做 git 快照。

## 8. 结论与建议依据

- 这条线**已经挣回了成本**：在尚未跑通端到端对拍时，仅 lexer/abi 两处的镜像与审读
  就产出 1 个主线级缺陷候选（K-1）+ 一族静默兜底登记 + 若干行为学发现——差分参照
  的价值在一期早期即实证成立，不是预期。
- 建议（候用户决定，不自动进二期）：继续推进的优先序 = 先完成集成轮（字节对拍 +
  边界转储 + 计时补测）拿到全管线差分基线，再按 §3 的价值序把语料面对 SAB 与
  checker 扩面；crate 图已按裁断定版，二期不再动拓扑，只在数据说话时重议 SAB 子 crate 化。
