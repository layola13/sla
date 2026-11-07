# ORC 计时与对拍线 — 三期阶段 A（协议定稿 + 对拍回归 + Zig 基线）

线位：ORC（辖区 scripts/**、results/p3/**、本文件）。用户裁决口径见
docs/COORDINATION.md §8「计时证据口径」（2026-11-07 10:11）：本线计时是
「tag 后 Zig vs Rust 替代裁决」的正式证据生产线。阶段 A = 协议 + Zig
基线；阶段 B（Rust 侧同口径测量 + 对比表）候 INT-2 落地后由协调线通知
启动，清单见 §6。

场地：Kaggle 场地三（hop 2202，hostname 66cf9c74eaf1，AMD EPYC 7B12，
4 核，MemTotal 32,869,296 kB）；全部构建/计时只在场地跑，本机只落文档
与原始数据。实测时刻 2026-11-07 约 10:17–10:25 CST。**负载注记**：测量
窗内三期其余线（INT-2/SAB-C/CHK3）在同一场地并行工作，绝对毫秒含并行
负载噪声；阶段 B 对比必须同窗重测 Zig 侧，不许直接拿本基线跨窗相减。

---

## 1. 同口径计时协议（定稿）

### 1.1 语料集（同口径集）

定义：双方管线今天都能端到端跑完的程序集。阶段 A 集 = e2e 三探针
（无 std 依赖），逐件 sha256（源文件）：

| 件 | 源 sha256 | 字节 | 现状 |
|---|---|---|---|
| e2e_a | c6b2f5606d258fdf5a462449ceec24fff26e62d0fced41220949781887a93333 | 279 | 双方跑完；产物差 D-O1 一处（§2） |
| e2e_b | ed9f128c98b8c2b933192e34d68e70548fa2587e135cb6282d552b43d4bb45e4 | 275 | 双方跑完；产物逐字节全等 |
| e2e_c | 4b2a69936a35ab23f8bf13b1e9e30d3518826e1ef4a7f7c04ec0621bbca59aee | 34 | 双方跑完；产物逐字节全等 |

候扩集（INT-2 解锁后逐件加入，源 sha256，fixtures/PROVENANCE.md 在案）：
01_hello d1b05644cab65721361bf2e16b153560c5feab041e23cd74b4cb759604ecaad2、
02_arith a30a4cd35e8d83257010260c4101d46a8a6baee78e247afe7fd94aaee78663ae、
03_if_else 13c97b3918764f30069bc6575b2f02bff3e213d53446231e33a9df42cd886d21、
04_while_sum abc8d1674e7cc6729d673be1647148564bfe34527378a7df0db3745c60213e47、
07_functions 6434df391eddb1cb3ce13bb4435c717df60e50da6621ea5bd790b342cc9c67c7。

扩集规则（预写、禁事后改）：① INT-2 线宣告某件 fixture 过 check 段且
全管线跑完的当轮，该件即入集，不挑件、不延后；② 每次扩集后，集合内
全体成员在同一窗口内重测两侧（冷/暖全口径），旧窗口数据降为历史记录、
不与新窗口混排；③ 入集件若任一侧跑不完，当轮登记缺口并暂缓入集，
理由原文入本文件修订记录。

### 1.2 编译速度口径

- 量：端到端墙钟，每轮新进程，整数毫秒（`date +%s%N` 前后差，方法与
  scripts/bench.sh 一致）。命令原文：Zig 侧 `sa sla build <f>.sla`
  （环境：`source /content/tools/env.sh` + `SA_STD_DIR=/content/sa_all/sci/sa_std`）；
  Rust 侧（阶段 B）`target/release/sla-oracle <f>.sla -o <out>`（默认
  build 模式，与 `sa sla build` 同程序口径，D-O2）。
- **冷**：每轮前清各侧缓存面后单跑。Zig 侧缓存面 = 工作目录
  `.sla-cache` + `~/.cache/sa/frontend`（逐轮 `rm -rf` 两处）。Rust 侧
  缓存面 = 无磁盘缓存（oracle 每进程从源文件全程重跑；cargo release
  产物在计时窗外预构建，不入计时）——阶段 B 如实按此列明，不对称处
  明说，不许伪造对称。
- **暖**：连跑，`SLA_NO_FRESHNESS_CACHE=1`，其余缓存全热（OS 页缓存、
  插件镜像、`~/.cache/sa/frontend` 均不清除）。**定此定义的原因是
  本线实测发现（2026-11-07，场地探针原文存 results/p3/zig-baseline/
  各件 profile_build.txt）**：同目录普通连跑第二轮起打印
  `[sla-profile] freshness gate: hit, front-end skipped`——新鲜度门
  命中时前端根本不执行；以普通连跑作「暖」测到的是短路而非编译。
  关掉新鲜度缓存后前端逐轮全程执行（探针 build3 无 freshness 行）。
- **repeat-hit（附带系列、独立命名）**：普通连跑（新鲜度门开），测
  短路路径本身；只作参考，**永不许引作编译时间**。
- 轮次与统计：每口径 ≥7 轮（本基线用 9 轮；repeat-hit 与宿主对照
  5 轮），报中位数 + IQR + 全距；原始每轮数据（ms + rc）逐轮落盘，
  不只存中位数。离群轮不删（中位数口径自带鲁棒性），原文保留。
- 两侧必须同场地、同窗口、同语料集；任一侧缺段（如合同段未补齐）
  时整表标注「尚非同口径」并附缺段清单，不出对比结论。

### 1.3 启动段拆分

可测到的段逐段报，测不到的明说：

- 宿主基线（仅 Zig 侧存在）：`sa --version` ×5 墙钟 = 进程启动 + 宿主
  本体（不走插件命令）。Rust 侧无宿主，此段为结构性不对称，B 表须
  单列，不许并入「编译」比较后不注记。
- Zig 本体分段：`SLA_PROFILE=1 sa sla check` 的 `[sla-profile]` 行
  （read source / source expand / parse / import expand / monomorphize /
  type check / load contracts summary），分辨率 1 ms；本语料规模下
  各段实测均为 0 ms（分辨率内），如实报「分辨率内不可分」，不外推。
- Rust 本体分段（阶段 B）：`SLA_RS_PROFILE=1` 的 oracle stage 行
  （seed / lex / parse / build_mode_prune / check / codegen_text，
  0.001 ms 分辨率）。
- 插件加载段（Zig 侧）：无独立计时工具，**测不到，明说**；只能以
  「端到端 − 宿主基线 − 本体分段」作残差描述，且残差还含 CLI 分发与
  IO，不命名为纯插件加载时间。

### 1.4 运行速度口径（按 COORDINATION §8 定稿）

产物（.sa 文本）逐字节全等 ⇒ 下游（汇编/链接/运行）面对同一字节流，
运行速度由构造相等，不再另测运行期。逐件以 sha 为证列入对比表；
**不全等件排除出运行结论**并列缺口（现集内 e2e_a 即此例：差 D-O1
清理序，运行结论对它不成立、不引）。理由：裁决问的是实现语言替代，
产物同字节时运行期无差可测；产物不同字节时差异属正确性/分歧面，
不属运行速度面。

### 1.5 Zig 基线定义（被测对象钉死）

- 宿主：`sa 0.1.8`，二进制 /content/sa_all/sci/zig-out/bin/sa，
  sha256 bca64cb80499f5fcebc267af3ffdff6f7a1a014c4b421bbfa808427da93165c3；
  sci 头 e82d286567ddd67630a7285684c6987f7e3c389b。
- 插件：装机件 /root/.local/share/sa_plugins/installed/sla/current/libsla.so，
  sha256 099ac8fb42897f0427f6926330ffc2a65a6a033030863f2da6fae58c4e8d8f70，
  37,150,096 B，与树内 zig-out/lib/libsla.so 同 sha（装机 = 该构建）。
  sla 头 6002b386c1a1a10057ce1c477c5b7573db447506。
- **构建形态：ReleaseFast**。证据链：场地 /content/tools/install.sh
  step 7 构建命令原文 `zig build -Doptimize=ReleaseFast`；体量
  37,150,096 B 与主线在案 ReleaseFast 体量（≈38 MB；Debug ≈116 MB）
  吻合。注记：build.zig 有 effectiveOptimizeForDevInstall 的 dev
  降级函数，若降级实际发生体量应为 Debug 量级，与实测体量矛盾，
  故不采降级读法。形态不同不许混测：阶段 B 若装机件 sha/形态变化，
  基线整表重测。

---

## 2. 对拍回归复跑（阶段 A 任务 1，实测）

命令：`bash scripts/orc_p3_regress.sh`（场地三，脚本与结果均双侧
md5 对版后拉回）。结果 results/p3/regress/（每件 run1/run2 双跑 +
zigwork/zigwork2 双跑 + sha256.txt + 三向 diff）：

| 件 | Zig 产物 sha256 | Rust 产物 sha256 | 判定 | 与二期 sha 对照 |
|---|---|---|---|---|
| e2e_a | 87adac2002a05cb7eec6054feaf6b44f3fa6c0e3e0af8739b85cf2e205f98890 | 38c71fab450628b3b1e31e5dd50e62b419829c11b27ea03d2f0dfb968c81ff26 | 差一处 = D-O1 | 两侧均与二期逐字同 sha |
| e2e_b | 8623e8d0b1326cddb50b4c2b23fd1de36412ff06eec866e2bf2a7862851c831e | 同左 | **逐字节全等** | 与二期同 sha |
| e2e_c | fe1902818c4a175e630cbe0d362f215693b0aeb7efbd6efdf7a5109d59b47ada | 同左 | **逐字节全等** | 与二期同 sha |

- b/c 仍逐字节全等，sha 与二期（REPORT §9.3）完全一致，无变化，
  无需停线登记。
- a 仍仅剩 D-O1：Zig 清理序 `!b !a !c !d`（哈希序，FB-020）对 Rust
  声明序 `!a !b !c !d`，仅 {!a,!b} 一对换位。两侧产物文件与二期
  d_o2 产物逐字节同 sha（本线 diff 直验 ZIG_SAME / RUST_SAME）；
  diff 文本表示本次写作 !a、二期写作 !b，是同一换位的两种等价
  最小 diff 表示，以 sha 为准、不作新分歧登记。
- 确定性：每件 Rust 双跑同 sha、Zig 双跑同 sha（diff rc 全 0）；
  基线冷跑 9 轮产物 sha 亦全同（results/p3/zig-baseline/*/cold_out_sha256.txt）。

## 3. Zig 基线实测（阶段 A 任务 3，实测）

命令：`bash scripts/orc_p3_zig_baseline.sh`（场地三）。原始每轮数据：
results/p3/zig-baseline/<件>/{warm,cold,repeat_hit,host_version}_ms.txt；
环境与被测对象原文：results/p3/zig-baseline/env.txt。单位 ms，
每格 = 中位数（IQR，全距），n：冷/暖 9、repeat-hit/宿主 5。

| 件 | 冷（清 .sla-cache + frontend 缓存） | 暖（新鲜度关、余缓存热） | repeat-hit（短路、仅参考） | 宿主 `sa --version` |
|---|---|---|---|---|
| e2e_a | 31（2，29–33） | 31（1，29–31） | 30（1，29–33） | 21（1，21–22） |
| e2e_b | 30（2，28–34） | 30（2，28–44） | 29（1，28–32） | 22（1，22–24） |
| e2e_c | 29（2，27–37） | 30（1，28–30） | 27（1，26–28） | 22（0，22–24） |

读数注记（均为实测内观察，不外推）：

1. 本语料规模下端到端由启动主导：宿主本体 alone 已 21–22 ms，
   端到端 29–31 ms；本体分段在 Zig profile 的 1 ms 分辨率内为
   0 ms（合同段 load contracts summary 亦 0 ms——三探针无导入，
   与一期 fixture 01 首触 61–81 ms 的合同加载不可同语）。
2. 冷 ≈ 暖：三探针无导入、无合同加载，清缓存的代价在本规模下
   不可分辨；此结论只对现集成立，扩集（fixtures 带 std 导入）
   后必须按 §1.1 规则全体重测，届时冷/暖差才有判读意义。
3. e2e_b 暖轮第 6 轮 44 ms 为单轮离群（同窗并行负载），原文保留，
   中位数不受影响。
4. 二期计时既有登记（REPORT §9.3）在此按纪律原样引用、仅作初步
   观测：同窗 7 轮中位 Zig 端到端 26 ms、Rust 管线 3–4 ms，
   **非同口径**（Rust 侧是轻线束、无插件加载/合同段），正式报告
   不许引用为结论。本基线 Zig 冷/暖中位 29–31 ms 与其 26 ms
   同量级、不同窗，不作差值解读。

## 4. 本线登记（移植/线束侧问题，非 Zig 缺陷）

- ORC-1（方法学，已修正）：首轮基线脚本以普通连跑作「暖」，实测
  证明新鲜度门命中后前端跳过（§1.2），首轮 warm 系列作废、按
  定稿定义重测；作废数据未入本文件任何表格。教训与主线「缓存
  掩盖新行为」同族：连跑计时前必须先证前端真的在跑。
- ORC-2（记录）：profile_build.txt 捕获的是 repeat 状态下的 build，
  内容为 freshness-hit 一行——它作新鲜度门的证据文件用，不作
  分段数据；分段数据以 profile_check.txt 为准。
- FB 号段 FB-033..FB-034：本阶段**未用**——未撞见新的 Zig 侧缺陷
  候选（e2e_a 哈希序为已在册 FB-020；新鲜度门为设计内缓存行为），
  号段退回协调线。

## 5. 阶段 B 执行清单（候协调线通知，不在本阶段执行）

前置：INT-2 线交付（5/5 fixtures 过 check 段）+ 协调线通知 +
场地仍为现头（sci e82d2865 / sla 6002b386；若头已变，先重跑 §2
回归与 §3 基线再测 Rust 侧）。

1. 按 §1.1 扩集规则把已解锁 fixtures 逐件入集，更新语料表 sha。
2. 同窗内：先重跑 scripts/orc_p3_regress.sh（全集对拍，sha 入表），
   再同窗重跑 Zig 基线（scripts/orc_p3_zig_baseline.sh，全集口径）。
3. Rust 侧同口径测量：新增 scripts/orc_p3_rust_timing.sh（辖区内），
   对全集逐件测冷/暖——Rust 侧冷 = 新进程 + 无磁盘缓存可清（如实
   注明）、暖 = 新进程连跑（oracle 无跨进程缓存，两口径预期同值、
   照实报）；分段以 SLA_RS_PROFILE=1 stage 行为据；原始轮次落
   results/p3/rust-timing/。
4. 出对比表（本文件 §7 增补）：逐件 × 冷/暖 × 中位数/IQR/全距，
   两侧原始数据路径齐备；启动段按 §1.3 单列（Zig 宿主基线 vs
   Rust 无宿主），本体分段并列（Zig 1 ms 分辨率注记保留）。
5. 运行速度结论按 §1.4：逐件列产物 sha，全等件写「由构造相等」，
   不全等件（至少 e2e_a）排除并列缺口。
6. 全表标注口径状态：同口径成立才出「编译速度差」结论行；
   否则标题明写「尚非同口径」+ 缺段清单。数字齐后由协调线汇入
   三期完工报告（COORDINATION §8 交付物）。

---

## 6. 阶段 B 实测（扩集 8 件 · 同窗双侧重测 · 对比表）

执行：§5 清单 1–6，脚本 scripts/orc_p3_phaseb.sh（场地三单跑，
2026-11-07 03:14:20–03:14:45 UTC，hostname 66cf9c74eaf1，与阶段 A
同机）。前置复核：场地头未变（sci e82d2865 / sla 6002b386）、装机
插件 sha 与阶段 A 基线逐字同（099ac8fb…，37,150,096 B，
ReleaseFast）——被测对象与 §1.5 钉死定义一致；按 §1.1 扩集规则②，
A 阶段 Zig 基线数字就此降为历史登记，本节两侧均为本窗重测。
原始数据：results/p3/phase-b/（env.txt / oracle_bin.txt /
regress/<件>/ / timing/<件>/ / stats.txt）。统计口径：中位数 +
IQR + 全距，n=9（repeat-hit n=5），IQR 为线性插值四分位距，
统计在场地以 python3 对原始轮次计算（stats.txt 原文）。

防陈旧二进制：计时/对拍前 `find crates -name '*.rs' -exec touch`
强制 cargo 重编（release，14.75 s，rc=0，cargo_build.log），被测
oracle sha256 95af0d4e…（2,097,280 B）；自证跑 e2e_c 产物 =
金值 fe190281…（oracle_bin.txt）。

### 6.1 扩集语料（同口径集，8 件，源 sha256 入档）

| 件 | 源 sha256 | 字节 | 入集依据 |
|---|---|---|---|
| e2e_a | c6b2f5606d258fdf5a462449ceec24fff26e62d0fced41220949781887a93333 | 279 | 阶段 A 集 |
| e2e_b | ed9f128c98b8c2b933192e34d68e70548fa2587e135cb6282d552b43d4bb45e4 | 275 | 阶段 A 集 |
| e2e_c | 4b2a69936a35ab23f8bf13b1e9e30d3518826e1ef4a7f7c04ec0621bbca59aee | 34 | 阶段 A 集 |
| 01_hello | d1b05644cab65721361bf2e16b153560c5feab041e23cd74b4cb759604ecaad2 | 287 | INT-2 + INT21 解锁 |
| 02_arith | a30a4cd35e8d83257010260c4101d46a8a6baee78e247afe7fd94aaee78663ae | 729 | 同上 |
| 03_if_else | 13c97b3918764f30069bc6575b2f02bff3e213d53446231e33a9df42cd886d21 | 429 | 同上 |
| 04_while_sum | abc8d1674e7cc6729d673be1647148564bfe34527378a7df0db3745c60213e47 | 522 | 同上 |
| 07_functions | 6434df391eddb1cb3ce13bb4435c717df60e50da6621ea5bd790b342cc9c67c7 | 544 | 同上 |

### 6.2 对拍回归（本窗，全集）

两侧全部 8 件 rc=0；每件 Rust 双跑同 sha、Zig 双跑同 sha
（确定性成立）；计时 36 轮/件的产物 sha 亦全程单一（timing/
<件>/*_out_sha256.txt 每系列 9 行同 sha）。**无新差**，无需停线。

| 件 | Zig 产物 sha256 | Rust 产物 sha256 | 判定 |
|---|---|---|---|
| e2e_a | 87adac2002a05cb7eec6054feaf6b44f3fa6c0e3e0af8739b85cf2e205f98890 | 38c71fab450628b3b1e31e5dd50e62b419829c11b27ea03d2f0dfb968c81ff26 | 差一处 = D-O1（FB-020） |
| e2e_b | 8623e8d0b1326cddb50b4c2b23fd1de36412ff06eec866e2bf2a7862851c831e | 同左 | **逐字节全等** |
| e2e_c | fe1902818c4a175e630cbe0d362f215693b0aeb7efbd6efdf7a5109d59b47ada | 同左 | **逐字节全等** |
| 01_hello | 0d6375acf0acca14a8824b8f96a91da3e8e16dba6bb335f46e03b06367817b26 | 同左 | **逐字节全等**（= INT21 金值） |
| 02_arith | 0b13f453f7221cbe8ff9efdd8497445cc05e18484c4afa1df8c1e49ede7a8128（= 金值） | fe38d48e80f6ed5af1bcf490f9ecd091c87edd5027ca87e3ef7347f47284e161 | 差一处 = D-C23 |
| 03_if_else | 5a92ee8240d1a1660bed6ba2be93af5e840298a8d0e26e5faf87c0f3fcf064a7 | 同左 | **逐字节全等**（= INT21 金值） |
| 04_while_sum | 60f814780a050c896d21fff3e46d04b1cb4bfae3164f0e0e7b941dd0768937dd | 同左 | **逐字节全等**（= INT21 金值） |
| 07_functions | 774e41d678658ff8a5308201c2349a1767b5bf4748971fc6fab1b5ba018d2dcc（= 金值） | d61ed5d980fe34cc2d638d679638276ef67358596cab9b44cc1475555034047c | 差一处 = D-C23 |

diff 实形（regress/<件>/diff_rust_vs_zig.txt，与 INT21 登记逐字
一致）：三件均为同一对位换位——Rust 声明序 `!a` 在前、Zig 哈希
序把 `!a` 放后一行：e2e_a `67d66 < !a / 68a68 > !a`（arith_values
出口清理）；02 `69d68 / 70a70`（同函数）；07 `44d43 / 45a45`
（add 出口清理）。字节数与行多重集均相等，仅 {!a,!b} 首对清理序
不同。e2e_a 的差为 D-O1（= checker D-C6 / FB-020）；02/07 为
D-C23（D-C6 的端到端实形，INT21 登记）。Zig 侧本窗产物与在册
金值逐件相符（fixtures 五件 expected + e2e 三件阶段 A sha）。

### 6.3 编译速度对比表（同窗、交错跑序）

跑序：第 r 轮内逐件交错四系列，奇数轮 Zig 先、偶数轮 Rust 先，
摊两侧负载尾。单位 ms，每格 = 中位数（IQR，全距），n=9。
百分比差 =（Zig − Rust）/ Zig × 100，即 Rust 侧端到端低多少，
**只在本表口径内成立（口径缺口见 §6.5，逐条列清）**。

| 件 | Zig 暖 | Rust 暖 | 暖差 | Zig 冷 | Rust 冷 | 冷差 |
|---|---|---|---|---|---|---|
| e2e_a | 25（1，24–26） | 2（1，2–4） | −92.0% | 26（1，24–27） | 3（2，2–5） | −88.5% |
| e2e_b | 25（2，24–27） | 3（1，2–4） | −88.0% | 27（3，25–29） | 3（1，2–5） | −88.9% |
| e2e_c | 25（1，24–27） | 2（1，2–4） | −92.0% | 26（1，25–27） | 2（1，2–4） | −92.3% |
| 01_hello | 37（3，35–92） | 12（2，12–15） | −67.6% | 89（2，87–114） | 12（1，11–13） | −86.5% |
| 02_arith | 26（2，24–36） | 3（1，3–6） | −88.5% | 27（1，24–30） | 3（1，3–5） | −88.9% |
| 03_if_else | 25（2，23–28） | 3（0，2–4） | −88.0% | 26（1，24–30） | 3（1，3–4） | −88.5% |
| 04_while_sum | 26（1，24–29） | 3（0，2–5） | −88.5% | 27（2，25–28） | 3（0，3–5） | −88.9% |
| 07_functions | 26（1，24–27） | 3（1，3–5） | −88.5% | 26（1，26–28） | 3（1，3–6） | −88.5% |
| **全集（中位数之和）** | **215** | **31** | **−85.6%（6.94×）** | **274** | **32** | **−88.3%（8.56×）** |

读数注记（实测内观察，不外推）：

1. 01_hello 是唯一本体段可分辨的件：Zig 暖首轮 92 ms 为计时
   工作目录首建的前端缓存预热（原文保留，中位数不受影响）；
   Zig 冷 89 ms = 每轮清 .sla-cache + frontend 缓存后宏索引与
   合同面从零重建（profile 的 load contracts 单项 9 ms，其余为
   宏索引/展开面，Zig 1 ms 分辨率下不可再分）。Rust 侧 01 冷≈暖
   （12/12）：oracle 无磁盘缓存，冷定义即新进程（§1.2 明列的
   不对称，非缺陷）。
2. 其余七件 Zig 冷≈暖（25–27）：清缓存代价在本规模下仍不可
   分辨，与阶段 A 观察 2 一致；差异全在启动形态（§6.4）。
3. repeat-hit 附带系列（仅参考，永不引作编译时间）：中位数
   e2e_a 26 / e2e_b 25 / e2e_c 25 / 01 26 / 02 25 / 03 24 /
   04 24 / 07 26——与暖口径同量级，本规模下新鲜度门短路不构成
   可分辨节省。
4. 负载：窗口起 load average 1.16–2.07、止 2.01（4 核），中位数
   口径抗扰；原始轮次全量在 timing/<件>/*_ms.txt。

### 6.4 启动段 / 本体段拆分（§1.3）

| 段 | Zig 侧 | Rust 侧 |
|---|---|---|
| 进程启动基线 | `sa --version` ×9：22（1，20–23） | oracle 无参数 usage 退出（rc=2）×9：3（0，3–4）——仅进程启动 + 参数处理，不走任何管线段，与左列**非同物**，只作各自地板参考 |
| 本体分段 | SLA_PROFILE 1 ms 分辨率：check 各段全 0 ms，唯 01 的 load contracts = 9 ms（roots=3 visited=30）；分辨率内不可再分 | SLA_RS_PROFILE 0.001 ms：七段合计 e2e_a 0.276 / e2e_b 0.245 / e2e_c 0.114 / 01 9.231（contracts 9.096）/ 02 0.542 / 03 0.513 / 04 0.485 / 07 0.523 ms（timing/<件>/rust_profile.txt） |
| 插件加载段 | **无独立计时工具，测不到**（协议原文口径）：端到端 − 宿主基线 ≈ 3 ms 的残差还含 CLI 分发与 IO，不命名为纯插件加载时间 | 无插件加载面（单体进程），此段不存在——结构性不对称，见 §6.5 |

### 6.5 口径缺口清单（表头注记，本节所有结论的前提）

同口径成立的部分：同机、同窗、同 8 件语料、双侧全管线端到端
（Zig `sa sla build` 对 Rust `sla-oracle` build 模式，同输出
形态 .sa 文本）、同统计口径。**以下为非同口径项，逐条列清**：

1. **进程形态不同（最大结构性不对称）**：Zig 侧 = 宿主进程
   （sa）+ 插件 .so 加载 + 宿主 machinery（CLI 分发、插件注册
   表）；Rust 侧 = oracle 开发线束单体进程（INT-2 合同段已含，
   非产品宿主形态）。启动地板实测 22 ms 对 3 ms，本表差值的大
   部分来自此形态差，而非管线本体（本体段两侧均 ≤0.6 ms，
   01 的合同段两侧同量级 9 ms）。
2. 冷口径不对称：Zig 冷 = 逐轮清两处磁盘缓存；Rust 无磁盘
   缓存可清，冷 = 新进程全程重跑。两侧冷定义均已在 §1.2 预写
   明列，不许伪造对称。
3. Zig 插件加载段无工具可测（§6.4），只能残差描述。
4. 分段分辨率不对称：Zig profile 1 ms、Rust 0.001 ms；本体段
   对比只到「01 合同段同量级、其余均在 Zig 分辨率内」为止。
5. 构建形态：Zig 插件 ReleaseFast 装机件；Rust 为 cargo
   release 构建——各为其产品/开发线束的现行形态，非同一优化
   管线产物。
6. 语料规模：源 34–729 B，本体段被启动段淹没（01 除外）；
   本表**不外推**到大程序/全 std 程序，裁决引用时必须连同
   本清单。
7. 禁止摘引：任何单侧数字不得单独摘出作「超越」表述；百分比
   差只在本表与本清单绑定时成立。二期 26 ms / 3–4 ms 仍仅为
   初步观测、非同口径，只存 §3 历史注记。

### 6.6 运行速度结论（§1.4）

产物逐字节全等 ⇒ 运行速度由构造相等（下游面对同一字节流），
覆盖 5 件，sha 为证：e2e_b 8623e8d0…、e2e_c fe190281…、
01_hello 0d6375ac…、03_if_else 5a92ee82…、04_while_sum
60f81478…。排除 3 件并注明：e2e_a（D-O1 / FB-020 清理序）、
02_arith 与 07_functions（D-C23 清理序）——其差异属正确性/
分歧面，不属运行速度面，对其不作运行结论。

### 6.7 本线登记

- ORC-B1（归档方法学，已处置）：本机 VM 写拦截伪影（AGENTS.md
  在册故障族）在拉回阶段发作——exec shell 向 results/p3/
  phase-b/ 的批量写多次报 EPERM 或静默落 0 字节。处置：本地
  镜像改逐文件经 muse.write 落盘并逐个与场地 md5 清单核对；
  timing/** 全部文件、regress 的 sha256.txt / diff*.txt /
  diff*.rc / rust2.stderr、顶层 provenance 文件均 md5 与场地
  逐字一致（cargo_build.log 转写有警告文本细差，其 rc=0 与
  Finished 事实由 oracle_bin.txt 与日志尾行双证）。本地未镜像：
  boundary dumps、zigwork*/work/ 工作目录、.sa 产物副本（由
  sha256.txt 钉死；fixtures 的 Zig 产物与仓内 fixtures/
  expected/ 逐字节同，e2e 产物与 results/p3/regress/ 在册副本
  同 sha）、rust1/rust2.rc 与 run-vs-run diff 的 .rc（其 .txt
  为空、rc=0 已由 sha 相等证明）。场地树 /root/sla-plugin-rust/
  results/p3/phase-b/ 保有完整原始集。
- FB 号段：本阶段**未用**——未撞见新的 Zig 侧缺陷候选（三处
  差均为已在册 D-O1/D-C23），不新登记。
