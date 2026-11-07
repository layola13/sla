# REPORT.md — Rust 版 sa_plugin_sla 差分实验 · 一期报告

状态时刻：2026-11-07 约 09:45 CST（第二任协调线更新）。本报告是**阶段性**的：
四条子系统线已交付三条半，未完成项逐项明示，不把中间状态说成验收结果。
本地仓仅本地 git，不建远端、不推送。

## 0. 本实验的目的（用户定调）

不是「要不要迁 Rust」，而是**差分**：用一套独立的 Rust 第二实现当参照系，
照出 Zig 实现自身发现不了的底层 bug。分歧是成果、不是失败；每处分歧先登记、
再分类（Zig 侧缺陷候选 / 本移植缺陷 / 有意为之），禁止把 Rust 侧悄悄改成迁就 Zig。

## 1. 一期交付物与状态

| 交付 | 状态 |
|---|---|
| ABI 摸底 docs/ABI_MAP.md | **完成**：双描述符符号（数据 + fn）、PluginDescriptor 64B 逐字段、宿主加载/校验/命令分发路径、Context 约 88B 手工镜像面、handler ABI（state field 40B / options 16B / result 48B、返回码语义、result_free 所有权）、命令面全表 |
| 分歧登记 docs/DIVERGENCES.md + docs/divergences/&lt;lane&gt;.md | **合编完成**：协调线 5（D-C1..C5）+ lexer/ast 9 + parser 13（D-P1..P13）+ checker 10（D-C1..D-C10）+ SAB 16（SAB-D01..D16），合计 53 条在册；两条头号候选已实证并入反馈环（FB-001/FB-002） |
| Crate 图定案 | **完成**：六家裁断 Q1=A+ / Q2=A（全文 ~/workspace/consult-rust-split/VERDICT.md），10 成员已落根 Cargo.toml，见 docs/COORDINATION.md §1 |
| sla-abi 全量镜像 + sla-plugin 锚点 | **完成（协调线欠账已补）**：abi 四模块镜像（布局按场地探针实测钉死，测试 5/5）；plugin 描述符双锚点已接线 + skills 静态表逐字镜像（其测试与 .so 符号验活候 workspace 全量编译放行，见 §7） |
| sla-oracle 边界转储 | **已跑通**：D-P3 seed 预扫描（协调线职责）+ seed/tokens/AST/checked/lowering/SAB 模型字节/文本六段转储 + 分阶段计时；generate 已接通（CheckInfo 适配） |
| 垂直切片对拍（3–5 程序逐字节） | **管线已接通、对拍未过**：workspace 全量构建通过、oracle 端到端可跑；五件 fixtures 与探针全数止于 checker 的 INT-1 误报（在场 return 被报 missing return，3 行最小复现已备，见 §7.1），其后还有 INT-2 合同段缺口挡 std 调用——这是本报告最大的未完成项 |
| 分阶段计时对比 | **半成**：Zig 侧基线已实测（§5）；Rust 侧管线虽接通但无成功端到端产物，未测（不拿失败路径计时冒充） |
| 700 行纪律 | **达标（盘点时刻）**：check_file_lines.sh 通过，78 个 .rs 全 ≤700 |
| 全 workspace 构建 | **10/10 crate 构建通过**；round4 全量测试全绿（59 passed / 0 failed，§4）；plugin .so 描述符双锚点 nm -D 验活 |

## 2. 这条差分线已经照出了什么

按价值排序（详情在 docs/DIVERGENCES.md）：

1. **K-1（=D-L2 = DEF-LEX-ESCAPE-EOF，Zig 侧缺陷，已实证确认）**：lexer 字符串/模板转义扫描在
   「未闭合字符串以 `\` 结尾」时 index 可推进到 `buffer.len + 1`，产出 `loc.end` 越界的
   token；下游按 loc 切片在安全构建下 panic、ReleaseFast 下 UB。Rust 借用/切片纪律
   **逼**出了这个点——Zig 侧自查很难看见，因为正常输入永不触发。Rust 钳制行为已裁决
   不迁就，列为已知有意分歧。**2026-11-07 协调线探针实证**：原 lexer 五例，
   尾单反斜杠形态 loc.end == len+1（两路皆然），对照恰为 len；模板路另有
   越界后不产 eof、原地重复 invalid 的附带现象。证据 docs/probes/dl2_probe.out.txt；
   反馈单 **FB-002** 已发。
1b. **D-CK1（=checker D-C1，Zig 侧缺陷，已实证确认）**：type_checker 的分支/循环
   状态快照按**名字**键控（saveScopeStates 由内向外覆盖同一扁平表），遮蔽绑定
   被合并——内层同名绑定在分支内的消费会被外层绑定的 Active 状态顶掉。
   协调线全管线复现（原插件 699f13d4）：无遮蔽对照报 UseAfterMove rc=1，
   遮蔽件 **rc=0 漏检**，机制预测逐项吻合。Rust 侧 BindingId 身份键控
   建模使该形态**不可表达**。证据 docs/probes/dc1_repro.out.txt；反馈单
   **FB-001** 已发。这是「检查器所有权记账」类缺陷的第一个全管线实证。
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
5. **sleb128 向量分歧（已结案）**：裁决为**本移植缺陷（测试向量笔误）**——
   原期望 `[179,159,127]` 是 -12365 的编码、被误作 -12345；实现产出
   `[199,159,127]` 经协调线场地探针（原 sci 编码器同输入）证实与 Zig
   逐字节一致。差分先红、再由双向证据结案，这正是它该起的作用；
   程序注记（向量改在书面裁决前约 31 秒）已如实登记于合编结案节。
6. **Context 布局实测（D-C1 量化）**：场地探针测得 Context = 72 B 且 Zig
   自动布局**重排字段**（json_mode 实际在偏移 68、broker_call 在 48，
   与声明序不同）——按声明序镜像会同尺寸错偏移。Rust 镜像已按实测序
   落盘；其间镜像自身还被布局测试抓出一个真错（`Option<*mut T>` 带
   判别标签使结构体整体膨胀），已改裸指针。差分纪律对移植自身同样有效。
7. **SAB-D01 参照系漂移（留笔）**：SAB 线首轮对拍差 1 字节，根因是**我方
   本地参照树 sci 滞后现头**（缺 is_variadic 字段），非任何实现错误；
   按现头修正后探针对拍 sha 全同。教训与主线「取材先对版」同族：
   差分实验的基准本身必须先对版，否则抓到的「分歧」是参照系的影子。

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
- round3（2026-11-07，--no-fail-fast，日志 workspace-test-20261107-round3.log）：
  lexer 5 / parser 12 / checker 16 / lowering 各套件 / sab 各套件全绿；
  abi 先 4 红（协调线镜像 Option<裸指针> 之误）改后 5/5；codegen-text
  未过编译（20 错误，见 §7.1）使 workspace 未合拢。
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
- Rust 现况（盘点时刻、仍在增长）：78 个 .rs、约 17,572 行。其中 lexer+ast 两 crate
  为全量镜像已交付，parser（3,173 行）/checker（3,648 行）已交付，
  lowering 与 codegen-text 合计约 6,800 行已落盘（text 待过编译），
  sla-abi/sla-plugin/sla-oracle 为协调线体量（约 1,900 行）。
- 拆分图：sab_codegen 的 20k 行按裁断 Q2=A 落为**单 crate 内模块族**
  （format/ registry/ dispatch/ layout/ drop_cleanup/ emit/，职责一级轴 + std 域二级轴，
  registry 按 std 域分文件卡 700 行）——互递归事实决定拆成子 crate 必须引入 Zig 中
  不存在的 trait 层、反而损害差分对位，故停在模块层（裁断依据见 VERDICT.md）。
  codegen.zig / lowering_rules.zig 同理只做 crate 内模块拆分。

## 7. 未完成项与场地状态（如实登记）

1. 垂直切片字节对拍未过——管线已接通（workspace 全绿、oracle 端到端
   可跑、plugin 双锚点已验活），卡点收窄为两处（DIVERGENCES 集成发现）：
   **INT-1** checker 对在场 return 误报 missing return（最小复现
   docs/probes/e2e_c.sla 三行，AST 转储证明 parse 无误，归 checker 线
   修复并补单测）；**INT-2** 合同加载/导入展开段在一期 crate 图中无主，
   fixtures 的 println/std 调用无从注册（候二期立项 .sai 合同解析 +
   展开接线）。INT-1 清零后：无 std 依赖程序先行 compare.sh 逐字节
   对拍 → bench.sh 计时同窗补测 → 本节销账。
2. 第二 ABI 锚点（sla_compile_handler / result_free）按设计**未导出**：
   handler 编译器尚无端口，导出桩会虚构原实现没有的行为；类型面已在
   sla-abi 备齐，候 handler 移植时接线。
3. parser/checker 的正式交付已收；lowering+text 线的 divergences/
   lowering-text.md 尚未落盘（代码已落、登记在途），合编候其补齐。
4. 场地：Kaggle 场地二（hop 2201）今日多次探针/构建均存活（已超名义
   hold），仍属随时可断；后继场地候主会话发放。场地一断，本线按规程
   停手登记、**不回落本机构建**。本机主本为唯一真源，已做 git 快照。

## 8. 结论与建议依据

- 这条线**已经挣回了成本**：在尚未跑通端到端对拍时，仅 lexer/abi 两处的镜像与审读
  就产出 1 个主线级缺陷候选（K-1）+ 一族静默兜底登记 + 若干行为学发现——差分参照
  的价值在一期早期即实证成立，不是预期。本轮又添 checker 的 D-CK1 全管线
  实证：两条头号候选均已按用户新政入**反馈环**（FB-001/FB-002），
  Zig 侧修复与 Rust 移植自此并行，不再等移植收官。
- 建议（候用户决定，不自动进二期）：继续推进的优先序 = 先完成集成轮（字节对拍 +
  边界转储 + 计时补测）拿到全管线差分基线，再按 §3 的价值序把语料面对 SAB 与
  checker 扩面；crate 图已按裁断定版，二期不再动拓扑，只在数据说话时重议 SAB 子 crate 化。
