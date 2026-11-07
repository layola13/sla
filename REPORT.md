# REPORT.md — Rust 版 sa_plugin_sla 差分实验 · 一期报告（附二期收编）

状态时刻：2026-11-07 约 09:50 CST（第二任协调线更新）。本报告是**阶段性**的：
四条子系统线**已全部交付**，集成轮在场地三（2202）推进；未完成项逐项明示，
不把中间状态说成验收结果。本地仓仅本地 git，不建远端、不推送。

**二期收编（2026-11-07 约 10:1x CST，三期协调线）**：二期四线 + 两件
收尾小线全部交付并经协调线收编——INT-1 修复销账、checker 扩围完成
（未覆盖清单仅剩 3 项近似/简化）、SAB B 组落地且 `generate_from_program`
真实驱动、oracle 全管线对拍达成（e2e_b/e2e_c 逐字节全等、e2e_a 仅剩
FB-020 一处）、分阶段计时同窗补测完成。workspace 测试 **137 passed /
0 failed**，分歧在册 **82 条**，反馈环累计 **18 单**（FB-001..FB-017 +
FB-020）。详情见 §9；§1/§7 已按二期状态更新。

## 0. 本实验的目的（用户定调）

不是「要不要迁 Rust」，而是**差分**：用一套独立的 Rust 第二实现当参照系，
照出 Zig 实现自身发现不了的底层 bug。分歧是成果、不是失败；每处分歧先登记、
再分类（Zig 侧缺陷候选 / 本移植缺陷 / 有意为之），禁止把 Rust 侧悄悄改成迁就 Zig。

## 1. 一期交付物与状态

| 交付 | 状态 |
|---|---|
| ABI 摸底 docs/ABI_MAP.md | **完成**：双描述符符号（数据 + fn）、PluginDescriptor 64B 逐字段、宿主加载/校验/命令分发路径、Context 约 88B 手工镜像面、handler ABI（state field 40B / options 16B / result 48B、返回码语义、result_free 所有权）、命令面全表 |
| 分歧登记 docs/DIVERGENCES.md + docs/divergences/<lane>.md | **合编完成**：一期 67 条（协调线 5 + lexer/ast 9 + parser 13 + checker 10 + SAB 16 + lowering/text 14）；二期续登 checker +7（D-C11..D-C17）+ SAB +6（D17..D22）+ oracle +2（D-O1/D-O2），**合计 82 条在册**；反馈环累计 18 单（FB-001..FB-017 + FB-020） |
| Crate 图定案 | **完成**：六家裁断 Q1=A+ / Q2=A（全文 ~/workspace/consult-rust-split/VERDICT.md），10 成员已落根 Cargo.toml，见 docs/COORDINATION.md §1 |
| sla-abi 全量镜像 + sla-plugin 锚点 | **完成（协调线欠账已补）**：abi 四模块镜像（布局按场地探针实测钉死，测试 5/5）；plugin 描述符双锚点已接线 + skills 静态表逐字镜像（其测试与 .so 符号验活候 workspace 全量编译放行，见 §7） |
| sla-oracle 边界转储 | **已跑通**：D-P3 seed 预扫描（协调线职责）+ seed/tokens/AST/checked/lowering/SAB 模型字节/文本六段转储 + 分阶段计时；generate 已接通（CheckInfo 适配）；二期接线 build 模式 @test 体清空（D-O2 销账） |
| 垂直切片对拍（3–5 程序逐字节） | **text 路已定谳 5/5**（lowering/text 线对拍台）。**oracle 全管线对拍二期达成**：INT-1 修复后，e2e_c/e2e_b 与真插件产物**逐字节全等**，e2e_a 仅剩 D-O1（清理序哈希序，FB-020）一处；5 件 fixtures 仍被 INT-2 挡在 check 段（逐件清单已立）——INT-2 是当前最大的未完成项 |
| 分阶段计时对比 | **二期补测完成（同窗实测）**：e2e 三探针 Zig 端到端中位 26 ms、Rust 管线 3–4 ms；两边分阶段本体均 sub-ms 级、端到端皆启动主导（口径差异如实注于 §9，不作编译速度结论） |
| 700 行纪律 | **达标（盘点时刻）**：check_file_lines.sh 通过，全部 .rs 全 ≤700 |
| 全 workspace 构建 | **10/10 crate 构建通过**；round5 全量测试（场地三、清单校验过的树）**64 passed / 0 failed**（§4）；plugin .so 描述符双锚点 nm -D 验活；**二期终验 137 passed / 0 failed**（§9） |

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
4. **D-C4 / D-C5（对拍驱动出的行为学发现）——已双结案**：lowering/text 线定层：
   导入列表是源导入的纯函数（按序 + 5 条固定导入，协调线先前的 test 可达性
   假设证伪、已在合编中更正）；两个「不可见消耗点」均有确切分配点——tmp_7 是
   void if 在 genExpr 末尾丢弃的 newTmp()（codegen.zig:15871），L_MERGE 是
   无条件分配、按需发射的标签（约 15744）。不是缺陷，是必须逐字镜像的确定性
   行为；这类点正是字节对拍的核心猎物，猎法（先红、定层、销账）全程留痕。
4b. **lowering/text 线 (c) 类七条（FB-003..FB-009）**：最重一条是 genStmt 的
   `else => {}` **静默吞未处理语句**（codegen.zig:10650，整段语句无声消失，
   与 SAB 吞错族同科）；其余含 `sum` 硬编码跳过实参释放（9830）、未知被调用
   名静默假定宏 EXPAND（约 15540）、println 未知类型只打 1 字节（约 2397）、
   println 长度双口径（约 2185/2195）、@test 空壳的流水线口径（7610 上游）、
   一处死分支（lowering_rules 6221）。text 发射面是 16k 行 codegen 的总入口层，
   静默兜底在此密度最高——与 D-C2 同为「成建制发现机制」的兑现。
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
- round4（场地二）：workspace 构建 10/10、测试 59 passed / 0 failed。
- round5（2026-11-07，**场地三 2202**，同步后 md5 清单全量校验 0 差异的树，
  日志 workspace-test-20261107-round5-venue3.log）：**64 passed / 0 failed**；
  其中 lane D 候复验命令 `cargo test -p sla-codegen-text --test fixture_parity`
  = **5/5 fixture 逐字节全等**（01/02/03/04/07）。同轮 D-CK1 以现头
  sla 6002b386 复证（floor/control rc=1、shadow rc=0），FB-001 证据升现头口径。
- 集成轮注意：全量测试须 `cargo test --workspace --no-fail-fast`，否则首红遮蔽全格局；
  同步必须带清单校验（场地三曾现「计数同、内容旧」的混合树，见 COORDINATION 同步协议注）。

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
- Rust 现况（2026-11-07 盘点）：84 个 .rs、18,235 行，最大单文件 581 行
  （全 ≤700）。lexer+ast / parser（3,173 行）/ checker（3,648 行）/
  lowering+text / sab（编码器 + emit 主干）四线均已交付；
  sla-abi/sla-plugin/sla-oracle 为协调线体量。
- 拆分图：sab_codegen 的 20k 行按裁断 Q2=A 落为**单 crate 内模块族**
  （format/ registry/ dispatch/ layout/ drop_cleanup/ emit/，职责一级轴 + std 域二级轴，
  registry 按 std 域分文件卡 700 行）——互递归事实决定拆成子 crate 必须引入 Zig 中
  不存在的 trait 层、反而损害差分对位，故停在模块层（裁断依据见 VERDICT.md）。
  codegen.zig / lowering_rules.zig 同理只做 crate 内模块拆分。

## 7. 未完成项与场地状态（如实登记）

1. ~~垂直切片字节对拍未过~~ **二期进展**：**INT-1 已修复销账**
   （根因 = Rust 侧自创 Zig 不存在的 missing-return 判定，按
   type_checker.zig:2596-2604 原形重写，真值双路实证）；无 std
   依赖程序对拍已完成（e2e_b/e2e_c 逐字节全等、e2e_a 仅剩 D-O1），
   计时同窗补测已完成（§9）。**INT-2 仍开口且为当前最大未完成项**：
   合同加载/导入展开段在 crate 图中无主，5/5 fixtures 的 println/std
   调用无从注册（逐件阻挡清单已立，候三期立项 .sai 合同解析 +
   展开接线，与 SAB D 组表面预载同批规划）。二期其余开口：
   checker 未覆盖仅剩 3 项（关联调用接收者推导全形态、match 守卫
   语义近似、plain-if 条件 integer 口径）；SAB 的 C 组（预扫/符号）
   与 D 组（std 表面）未接线（driver 调用点均响亮报错）、test sig
   的 llvm_name 未设（SAB-D21 附记）。
2. 第二 ABI 锚点（sla_compile_handler / result_free）按设计**未导出**：
   handler 编译器尚无端口，导出桩会虚构原实现没有的行为；类型面已在
   sla-abi 备齐，候 handler 移植时接线。
3. 四线交付已全齐（lowering/text 的 divergences/lowering-text.md 已落盘
   并合编，D-T1..D-T14；其接线注记五条已由协调线逐项处置：CheckInfo
   地址键契约生效、reachability 桩归 lowering 所有列二期、未用 checker
   依赖已删、主索引已补行、@test 清空职责已注记）。
4. 场地：**已热切换至场地三**（hop 2202，hold 至约 19:43 CST，现头
   sci e82d2865 / sla 6002b386 + Rust 1.99.1 预装）；切换中发现并修复
   同步协议漏洞（覆盖解压 + 预置旧快照交叠 → 3 文件旧版/缺失而计数
   仍同），sync_to_venue.sh 已加擦除 + md5 清单全量校验。场地二
   （2201）已到期弃守。场地断仍按规程停手登记、**不回落本机构建**。
   本机主本为唯一真源，已做 git 快照。

## 8. 结论与建议依据

- 这条线**已经挣回了成本**：在尚未跑通端到端对拍时，仅 lexer/abi 两处的镜像与审读
  就产出 1 个主线级缺陷候选（K-1）+ 一族静默兜底登记 + 若干行为学发现——差分参照
  的价值在一期早期即实证成立，不是预期。本轮再添：checker 的 D-CK1 全管线
  实证（现头复证）、lowering/text 的 (c) 类七条、text 路 5/5 逐字节对拍定谳。
  反馈环累计 **9 单**（FB-001..FB-009），Zig 侧修复与 Rust 移植自此并行，
  不再等移植收官。
- 建议（候用户决定，不自动进二期）：继续推进的优先序 = 先完成集成轮（字节对拍 +
  边界转储 + 计时补测）拿到全管线差分基线，再按 §3 的价值序把语料面对 SAB 与
  checker 扩面；crate 图已按裁断定版，二期不再动拓扑，只在数据说话时重议 SAB 子 crate 化。

## 9. 二期收编（2026-11-07，三期协调线）

二期线组：checker 线（INT-1 → 扩围）、SAB 线（B 组）、oracle 线
（INT-1 落地后启动），加收尾小线两件（SAB-D17 双轨镜像、oracle
D-O2 接线）。场地三（2202）全程承载，开工前全量同步 md5 清单
0 差异、基线 64/0 复现后开线；各线只同步自家路径、逐次双确认。

### 9.1 checker 线

- **INT-1 销账**：根因是移植侧自创判定（见 §7.1 与 DIVERGENCES
  集成发现节）。修复后 checker 24/24，补 8 条「源码 parse → check」
  形态单测（含把 Zig「非表达式尾/空 body 在检查器层放行」的真实
  口径钉死，防 INT-1 以「补全」之名复活）。
- **扩围完成**：未覆盖清单逐项攻下——switch/match 应用层（多路
  合并改 BindingId 键控）+ match/switch 终止形态、if-let 链、
  let_destructure + 解构消费站点（D-C7 清单最后一处）+ let_else、
  泛型实例化（镜像「检查器无此检查」本身）、三张侧表（实证 try
  无独立表、写共享 cleanups，照 Zig 结构合并）、enum 字面量、
  slice 表达式、导入宏、trait/impl 契约、运算符重载、dyn/slice
  强转（附带修正一期偏差：const 臂误消费初值源）、闭包字面量、
  sum 的 slice 形态；关联调用为大部完成。checker **80/80**
  （24 → +56），真值双路（Zig 源码 + 真插件 12 探针）。
- 新分歧 D-C11..D-C17；反馈单 **FB-010..FB-014**（解构无 Copy
  豁免、switch 模式零检查、generic_func_ref 裸错误、pattern
  writable/is_const 反转、match 推断 OR 条件改写类型）。
- 确切剩余：① 关联调用接收者类型推导全形态；② match 守卫绑定
  可见性/消费语义近似；③ plain-if 条件接受 integer（一期既有
  简化，Zig 收 boolean/integer/infer），均已登记候后续线。

### 9.2 SAB 线

- B2 types（369 行）/ B3 state（255 行）/ B4 driver（642 + 532 行）
  落地，`generate_from_program` 改真实驱动（签名未动）：用户函数 +
  函数间调用程序端到端 334 B 与手工模型逐字节相等；while/if 控制
  流件 1184 B 双跑确定性一致；@test + panic 守卫件已钉 sha。
- std 表面（D 组）与 C/F/G/H/I 未接线调用点全部响亮报错（组别 +
  Zig 行段），无静默跳过、无回落文本路。sab **17/17**。
- 新分歧 SAB-D17..D22；反馈单 **FB-015..FB-017**：逃逸闭包按
  AutoHashMap 迭代序（指针哈希序）发射（FB-016，产物字节序本质
  不确定）、panic 多参静默写死 "1"（FB-017）、sig.id 双轨
  （FB-015）。
- **收尾 D17（真产物裁决）**：双轨规则钉到行（:5964 入表前以两表
  长度和赋值、test 双计数跳号、:6008 decl 操作数走 intern），并以
  真插件 `sa sla sab build` 产物字节级验证：multi.sab（662 B）
  sig.id 0/1/2 vs 操作数 0/5/9；multi2.sab（573 B）sig.id 0/1/**3**
  跳号实锤。端口已逐字镜像，driver_e2e 逐件重钉并给出字段级解释；
  一期探针 76B/427B sha 维持全同。

### 9.3 oracle 线

- **全管线对拍达成**（与本窗现跑真插件产物比）：e2e_c 逐字节
  全等（sha fe190281…）、e2e_b 在 D-O2 接线后逐字节全等
  （sha 8623e8d0…）、e2e_a 仅剩 D-O1 一处。此为实验首个
  AST 驱动端到端对拍点。
- **D-O1 = FB-020**：return 清理序 Zig 为哈希序 `!b !a !c !d`、
  Rust 为声明序——checker D-C6（清理名单 = StringHashMap 迭代序）
  的首个字节级实证；非 Copy 类型 drop 序可观察。Rust 按发射纪律
  不迁就，已落反馈单候主线处置。
- **INT-2 阻挡清单**：5/5 fixtures 全死于 check 段 println 调用
  （rc=5），01 另有 str_eq 第二挡，逐件在案。
- **计时同窗实测**（7 轮中位，e2e 三探针）：Zig 端到端 26 ms、
  Rust 管线 3–4 ms；分阶段本体两边皆 sub-ms 级（Rust 各段
  0.002–0.111 ms；Zig 探针 profile 分辨率内 0 ms）。口径注记：
  两边端到端均启动主导，Rust 侧是轻线束（无插件加载/合同段），
  与 sa 宿主非同一起跑线——本表只作实测登记，不作编译速度结论；
  一期 ±20% 预估至此仍**未证实未证伪**（可比口径不存在），
  不引用。fixture 01 contract load 同窗补测：首跑 61 ms、
  次跑 9 ms（Rust 侧该段不存在，记未测）。

### 9.4 二期总账

- 测试：workspace **137 passed / 0 failed**（64 → +73：一期基线
  之上 INT-1 +8、checker 扩围 +56、SAB +7、oracle +2）；100 个
  .rs 文件全 ≤700 行（最大 driver.rs 642）。
- 分歧在册 **82 条**（一期 67 + 二期 15）；反馈环累计 **18 单**
  （FB-001..FB-017 + FB-020），全部候主会话确认后与 Zig 侧修复
  并行（常设政策）。
- 三期候选（按差分价值序）：INT-2 合同段立项（解锁 5 fixtures
  全管线 + SAB D 组表面）→ SAB C 组预扫/符号 → checker 剩余
  3 项 → SAB-D21 的 test llvm_name → handler 活体对拍
  （§3 价值序第 3 位，一期 §7.2 的第二 ABI 锚点候此接线）。

## 10. 三期收编（2026-11-07，三期协调线）

三期线组：INT-2 合同段线（+ 后续 INT21 检查器臂线）、SAB-C 线、
SAB-D 线、CHK3 检查器线、HND 接线评估线、ORC 计时线（阶段 A
协议 + 阶段 B 实测）。场地三（2202）全程承载；协调裁决 D-A2
先行定 crate 归属：不开新 crate、不加新依赖边——.sai 合同解析
归 sla-parser 新模块、import 展开编排归 sla-plugin 接线（oracle
消费同一接线）、注册只走既有 `Checker::register_extern_func`。
集成轮由协调线亲跑：全量同步 md5 清单 463 文件 0 差异、过时
钉子测试按 INT-2 预登记更新、driver_e2e.rs 765 行按 700 行规
拆为 600 + 362 两文件（测试名/数不变）。

### 10.1 INT-2 合同段（INT-2 + INT21 两线）

- 落地：parser `contract.rs`（340 行）+ `imports.rs`（512 行，
  解析序照 Zig 镜像；硬编码 `/home/vscode/...` 根按 D-C3 有意
  不移植）+ `contract_load.rs`（326 行）；plugin `contracts.rs`
  只经 register_extern_func 注册、checker 本体未动；oracle 管线
  增合同段（`03a_contracts.txt` 转储，加载失败 → 退出码 5，
  对齐 Zig「Import Error」）。
- **关键分层发现 INT2-1**：`println`/`str_eq` 不是合同 extern，
  而是 Zig 检查器的**名字特判臂**（isPrintlnCall 任意元数
  ~:4268、isStrEqCall 恰 2 参 string-like ~:4240，先于用户函数
  解析、不可遮蔽；场地探针实证）。INT-2 线拒绝以 extern 伪注册
  （会伪造元数/类型语义），登记为检查器辖区缺口；INT21 线随后
  以 `check_builtin.rs`（398 行）把全部名字特判臂镜像落地
  （vec/len/str_eq/format/hash/debug/println/catch_unwind，怪癖
  逐条探针钉死：如 `len(42)` 在 Zig rc=0、实参类型不查）。
- **fixtures 5/5 全管线 rc=0**：01/03/04 与真插件产物**逐字节
  全等**（sha 0d6375ac… / 5a92ee82… / 60f81478…，双侧核验）；
  02/07 各差恰一处清理序对位（`!a!b` 对 Zig `!b!a`，字节数与
  行多重集相等）= **D-C23**，即 D-C6/FB-020 的端到端实形（Zig
  哈希序不确定性，Rust 按纪律守声明序）。01 全模式亦 rc=0
  （str_eq 第二挡销账）。二期 §9.3 的 INT-2 阻挡清单逐件销账。
- **A0 描述符活体对拍完成**：handle_command 对 help / skills
  [--json] / stability schema|verify / init / 未知命令的真实
  实现，在真 sa 0.1.8 宿主双沙盒矩阵 **33/33 零差异**（金值同
  窗复产：skills --json sha256 efbe756e…、stability schema
  99bf7d04…、init 三文件 sha 全同）。
- 协调裁决登记（四期接线项，均已证非阻挡）：INT2-2
  ExternSig.return_fallible 原子落地条件；INT2-3 Zig 的
  layout_defines 表全仓无读者（只写 + deinit，惰性元数据）；
  oracle 5 行注册垫片待删（oracle→plugin 边图上已许可）。

### 10.2 SAB 线（C 组 + D 组）

- C 组落地：C1 prescan_consts（657 行）/ C2 prescan_bindings
  （582）/ C3 symbols（541），D19 在册 C 组响亮点在驱动内全关。
- D 组落地（INT-2 解锁后）：D1 surface_load（672）/ D2 layout
  （593）/ D3 copy_facts（436）/ D4 surface_query（533）/
  D5 surface_preload+scan（447+353），驱动步 2/3 接线；无表面
  程序在规则开/关下逐字节同（接线发射中立已证）。端到端死亡
  点现精确位于 gen_call 表面展开 → F 组片段机器（已以 trait +
  响亮默认隔离）。真插件金值在册（println 探针 1002 B
  f7ef7511…、Vec::new 798 B 131c90aa…）。sab **73/0**。
- 反馈单：**FB-025**——顶层 const 折叠 `i64::MIN / -1` 无守卫，
  真插件在**文本路与 SAB 路双双 SIGFPE 核心转储 rc=136**
  （sab_codegen 与 lowering_rules 机制双生）；Rust 用 checked
  运算拒绝折叠。**FB-026**——零规则静默（D02 家族）。新分歧
  SAB-D23..D28（D23/D24 移植类已修、D25/D28 Zig 候选、D26/D27
  移植类）。

### 10.3 checker 线（CHK3）

- checker **118/0**（80 → +38），后随 INT21 臂线达 **146/146**。
  关联调用接收者推导全形态落地（新 check_method.rs，6 形态族，
  探针钉死）；match 守卫裁定为**结构等价、无代码变更**（10 探针：
  Zig 无守卫失败回滚、效应进多路合并，二期同帧近似语义相等，
  9 条管线测试钉死）；plain-if 三形态（boolean / legacy-integer /
  infer）完成，附带发现：现头 `int` 注解从不产生 legacy integer
  （注解层事实）。新分歧 D-C18..D-C23。**无新反馈单**——FB-029..
  032 两轮未用退回（所探 Zig 行为全部自洽）。

### 10.4 handler 接线评估线（HND）

- **重大发现 FB-035**：第二 ABI `sla_compile_handler` 在现头
  **出货插件中不存在**（场地三法实证：装机 + zig-out 两份
  libsla.so 的 nm -D 只有 2 个描述符锚点、strings 计数 0、
  dlopen False；sci/src grep 消费者 0；plugin_handler.zig 只有
  一个 part1 提交、唯一消费者是其自家单测）。此与本仓 ABI_MAP
  §1「实测」表矛盾——协调线亲验 zig-out 后已在 ABI_MAP 加带
  日期更正注（原文保留）。两读法候主线裁断：A = 预期 ABI 在
  构建中静默丢失；B = 遗迹、应退役。
- 另落 **FB-036**（handler_bridge 把 IO/解析失败吞成
  InvalidHandlerOutput）、**FB-037**（硬编码 extern 前缀 3 符号，
  未声明表面上限，观察级）、**FB-038**（check 命令内联复刻管线
  与 plugin_compile 双路漂移面，结构级、未证实）。
- 裁决：面 B（compile_handler 活体对拍）**不入三期**（无活体
  参照侧、依赖 monomorphizer/source_expand 未移植），候 FB-035
  主线裁断、四期候选；面 A 切片 A0 入三期并已由 INT-2 线完成
  （见 §10.1）。

### 10.5 同口径对比报告（ORC 阶段 A 协议 + 阶段 B 实测）

用户 2026-11-07 10:11 裁决的正式证据节。全文与原始数据：
docs/phase3/oracle-timing.md §1（协议）+ §6（阶段 B），
results/p3/phase-b/（231 文件）。阶段 A 方法学捕获：`sa` 前端
新鲜度门使朴素复跑整段跳过前端（日志「freshness gate: hit,
front-end skipped」）——协议定暖口径 = `SLA_NO_FRESHNESS_CACHE=1`
实跑，普通复跑另列 repeat-hit 系列、永不引作编译时间。

阶段 B：场地三单窗（2026-11-07 03:14:20–03:14:45 UTC）双侧
全重测（头未变 sci e82d2865 / sla 6002b386、装机插件 sha 与
A 基线逐字同 099ac8fb…；A 阶段数字按协议降为历史）。语料扩
至 8 件（e2e_a/b/c + fixtures 01/02/03/04/07，源 sha 入档）。
对拍回归先行：8/8 rc=0、两侧双跑确定性成立、36 轮/件产物
sha 全程单一；逐字节全等 5 件（e2e_b 8623e8d0…、e2e_c
fe190281…、01 0d6375ac…、03 5a92ee82…、04 60f81478…），差
一处 3 件均为同一 {!a,!b} 清理对位换位（e2e_a = D-O1、02/07
= D-C23），无新差。

编译速度（中位数，ms；IQR/全距与原始轮次在原文 §6.3/§6.4）：

| 件 | Zig 暖 | Rust 暖 | Zig 冷 | Rust 冷 |
|---|---|---|---|---|
| e2e_a | 25 | 2 | 26 | 3 |
| e2e_b | 25 | 3 | 27 | 3 |
| e2e_c | 25 | 2 | 26 | 2 |
| 01_hello | 37 | 12 | 89 | 12 |
| 02_arith | 26 | 3 | 27 | 3 |
| 03_if_else | 25 | 3 | 26 | 3 |
| 04_while_sum | 26 | 3 | 27 | 3 |
| 07_functions | 26 | 3 | 26 | 3 |
| **全集（中位数之和）** | **215** | **31（−85.6%，6.94×）** | **274** | **32（−88.3%，8.56×）** |

分段拆分：Zig 宿主基线 `sa --version` 22 ms；Rust 启动代理
（usage 退出）3 ms。本体段：Zig 1 ms 分辨率下全 0、唯 01 合同
加载 9 ms；Rust 七段合计 0.11–0.54 ms、01 为 9.23 ms（合同段
9.10）——**差值主体是启动形态而非管线本体**，两侧本体同量级。

运行速度结论（§1.4 口径：产物逐字节全等 ⇒ 由构造相等）：
覆盖 5 件（e2e_b、e2e_c、01、03、04，sha 如上）；排除 e2e_a
（D-O1/FB-020）、02/07（D-C23）——其差异属分歧面、不作运行
结论。

**口径缺口清单（结论与之绑定，原文 §6.5）**：① 进程形态不同
（最大不对称）：Zig = 宿主 + 插件加载 + 宿主 machinery（地板
22 ms）；Rust = oracle 开发线束单体进程、合同段已含（地板
3 ms）。② 冷口径不对称：Zig 冷清两处磁盘缓存；Rust 无磁盘
缓存、冷 = 新进程（协议预写明列）。③ Zig 插件加载段无计时
工具、测不到，只作残差描述（端到端 − 宿主 ≈ 3 ms，含 CLI
分发与 IO）。④ 分段分辨率不对称（Zig 1 ms vs Rust 0.001 ms）。
⑤ 构建形态各异（ReleaseFast 装机件 vs cargo release），各为
现行形态。⑥ 语料 34–729 B、启动主导，不外推大程序；禁止单侧
数字单独摘引作「超越」；二期 26 ms / 3–4 ms 仅作初步观测、
非同口径，不作结论引用。

**本节结论口径（如实）**：Rust 侧段面已补齐（合同段在内），
不存在「缺段」意义上的非同口径；但进程形态差使端到端差值
主要度量的是启动形态而非管线本体——在此语料规模上，**不能**
据本表宣称「Rust 全面超越 Zig」或据以直接触发替代裁决；本表
是该裁决的正式计时证据，其有效结论为：同机同窗同语料下
Rust 线束端到端显著更低（暖 −85.6% / 冷 −88.3%）、本体段两侧
同量级（01 合同段 9 ms 对 9.23 ms）、运行速度在 5 件逐字节
全等件上由构造相等。产品形态（Rust 版宿主/插件）的同口径
复测候四期。

### 10.6 三期总账与剩余面

- 测试：workspace **294 passed / 0 failed**（137 → +157）；
  129 个 .rs 文件全 ≤700 行。分线：checker 146/146、sab 73/0、
  A0 活体矩阵 33/33、fixtures 5/5（3 件逐字节全等 + 2 件
  D-C23）。
- 分歧在册 **106 条**（82 → +24：SAB-D23..D28、D-C18..D-C23、
  INT2-1..INT2-8、A0-1..A0-4）；反馈环累计 **24 单**，三期
  新增 6 单：FB-025（const 折叠 SIGFPE 双路）、FB-026（零规则
  静默）、FB-035（handler ABI 不存在）、FB-036、FB-037、
  FB-038，全部候主会话确认后入主线队列（常设政策）。
- 剩余响亮未接线面（如实登记）：SAB E–I 组（当前端到端死亡
  点 = F 组片段机器，gen_call 表面展开处）、import 宏 /
  workspace 包 / @expand_tuple、`.sla` 函数体内联展开、描述符
  管线命令、D-C22 关联块前裸名段（Some/Ok/Err/monitor/
  future__/poll__/waker/va_next，无 fixture 触达）、handler 面 B
  （候 FB-035 裁断）。协调裁决延后项 3 件见 §10.1 末。
- 场地注记：场地三 hold 至 2026-11-07 约 19:43 CST；全部交付
  物在本地主本，场地消亡不丢成果。三期快照 id 见本次提交。
