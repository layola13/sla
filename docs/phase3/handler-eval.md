# HND 评估 — handler 活体对拍（第二 ABI 锚点）接线评估

线位：三期 HND（只读评估线，COORDINATION §8）。评估时刻：2026-11-07。
参照树：本地 `~/workspace/local-sa/sa_plugin_sla`（git 头 `3de91932`，文件日期
2026-10-26）+ 场地三（2202）现头树 `sla 6002b386 / sci e82d2865 / sa 0.1.8`
抽查对版（plugin_handler.zig 导出行 50/125、plugin.zig 再出行 25/26 两树一致；
行号以下以本地树为准，场地抽查不一致处单独注明）。
Rust 侧：`~/workspace/sla-plugin-rust` 快照 `1b0518c` + 场地 release 构建产物。
本线未改任何代码文件，未同步、未提交；探针只在场地 `/tmp/hndprobe` 沙盒内
运行真宿主命令。

---

## 0. 先行结论（详证见 §1–§4，建议展开见 §5）

**任务书把两个不同的 ABI 面混称作「handler」；拆开后结论不同：**

- **面 A — 描述符 `handle_command` 命令面**（`plugin.zig:runSlaCommandAbi` →
`plugin_commands.runSlaCommandImpl`）：这才是 REPORT §3 第 3 位所指
「宿主加载 + skills/check/build 命令可用真宿主做活体对拍」的面。Rust 侧
现状 = 响亮 Failed 桩（`crates/sla-plugin/src/lib.rs:62-76`）。其中
**无管线命令子集（help / skills / stability / init / 未知命令）可低成本
纳入三期**（§5 最小切片 A0），与 INT-2 / 单态化 / SAB D 组零耦合。
- **面 B — 第二 ABI `sla_compile_handler` / `sla_compile_handler_result_free`**
（`plugin_handler.zig` + `handler_bridge.zig`，状态处理函数编译）：
**不纳入三期、候四期**。决定性理由有三：① 重大项 HND-C1（§4.1）——
Zig 现头出货的 `libsla.so` **实测根本不导出这两个符号**，且 sci 宿主
源码内**没有任何消费者**，「活体对拍」在参照侧没有活体；
② 其管线依赖的单态化（monomorphizer，Zig 1,905 行）与 source_expand
（450 行）在 Rust 侧**整段无主、无移植**，不归 INT-2 辖区；
③ handler 注入绑定（checker/codegen 的 injected bindings）在 Rust
checker/codegen-text 中无对应 API。

---

## 1. Zig handler 面全集

### 1.0 两个面的界定（术语钉死）

| | 面 A：handle_command | 面 B：sla_compile_handler |
|---|---|---|
| 入口 | `PluginDescriptor.handle_command`（`plugin_api.zig` PluginDescriptor，偏移 40） | 独立导出符号（设计上）`sla_compile_handler` / `sla_compile_handler_result_free` |
| Zig 实现 | `plugin.zig:218-252 runSlaCommandAbi` → `plugin_commands.zig:207 runSlaCommandImpl`（617 行） | `plugin_handler.zig:50-132`（ABI 壳，169 行）→ `handler_bridge.zig:239-296 compileHandlerWithSupport`（352 行） |
| 输入形态 | argv 字符串数组（`argv[0]`=宿主名、`argv[1]`=`sla`/`slab`、`argv[2]`=子命令）+ `Context`（含 broker）+ 双 HostStream | 裸指针+长度：handler 名、handler 源码、状态字段数组（40 B/个）、options（16 B，可 null）、out 结果结构体（48 B） |
| 输出形态 | stdout/stderr 文本流 + `out_code: u8` + AbiStatus 返回码；文件产物副作用 | result 的 body/support 两段 SA 文本（c_allocator 分配、所有权移交宿主、`result_free` 回收）；失败时 error_name 静态串 |
| 用 Context/broker | 是（仅经 freshness 缓存，§1.3） | **否**（签名里根本没有 Context） |
| JSON | 仅 skills/stability 两命令的输出形态（§1.1） | 无 |
| 宿主消费者 | sci `plugins.zig` 命令分发（ABI_MAP §6） | **现头 sci 内零引用**（实测，§4.1） |

任务书背景段「宿主经 broker 调命令、JSON 进出」描述的是面 A 的局部特征
（broker 只服务 freshness 缓存、JSON 只出现在两个命令），不适用于面 B。

### 1.1 面 A 命令清单（runSlaCommandImpl 分发表，plugin_commands.zig:207-617）

分发规则：`args.len < 2` → 返回 null（宿主记 unknown_command）；
`args[1]=="slab"` → 直达 SAB 子命令集；`args[1]!="sla"` → null；
`args.len < 3` → stderr 用法 + rc 1。未知命令 →
stderr `Unknown sla command: {cmd}` + rc 1（plugin_commands.zig:613-616，
场地实测 rc=1、原文一致）。

| 命令 | 输入 | 输出（stdout/stderr） | 副作用 | 管线依赖 |
|---|---|---|---|---|
| `help` | 无 | 固定用法文本 → **stderr**，rc 0（plugin_commands.zig:221-243；场地实测 rc=0） | 无 | 无 |
| `init [path]` | 可选路径，默认 `.` | `Initialized SLA binary project: {path}` + `Entry: {path}`（plugin_cli.zig:177-243） | 建目录 + 独占创建 `sa.mod` / `src/main.sla` / `.gitignore`（已存在 → File Error rc 1） | 无 |
| `skills [--json]` | `--json` 或 ctx.json_mode | JSON 模式：单行 JSON → stdout（plugin_skills.zig:64-79）；文本模式：stdout 路径清单 + 分节文本 | **文本模式有副作用**：向 cwd 写 `.codex/skills/sla/SKILL.md` 与 `.claude/skills/sla/SKILL.md`（plugin_skills.zig:131-153） | 无 |
| `stability schema` | `[--json]`（被接受、无区别） | 静态 `schema_json` 常量 + `\n` → stdout（stability_metadata.zig:4-52，plugin_cli.zig:245-281） | 无 | 无 |
| `stability verify <manifest.json> [--json]` | 清单文件路径 | 校验报告 JSON 或文本 → stdout；valid → rc 0，否则 rc 1（plugin_cli.zig:283-323，stability_metadata.zig:111-386） | 只读清单文件 | 无（JSON 解析为 Zig std.json） |
| `build [file] [-p pkg] [--out f]` |.sla 源文件 | 成功行 `Sla Compiler: Successfully compiled {file} to {out}.` → stdout | 写 `{file}.sa`（或 --out 指定）；经 freshness 缓存（§1.3）；可能写 `.sla-cache/` | **全管线**：compileSlaToSaString（plugin_compile.zig:576-807） |
| `check [file] [-p pkg]` |.sla 源文件 | 成功行 `Sla Compiler: Successfully parsed and verified syntax and types of {file}.` → stdout；失败诊断 → stderr + rc 1 | 只读源（无产物；无 freshness） | 全管线内联复刻（plugin_commands.zig check 分支：read→expand→parse→import 展开→mono→load contracts→check，**不走 plugin_compile 主干**，见 HND-C4） |
| `sab build [file] [--out f.sab]` |.sla 源文件 | 成功行含 managed 缓存路径 → stdout | 写 managed SAB（`.sla-cache/sab/{stem}-{wyhash(file串)}.sab`，plugin_sab_paths.zig:21-26）+ 可选 --out 副本 | SAB 管线 compileSlaFileToSab（plugin_compile.zig:855-929）+ freshness |
| `sab workspace` | 无源文件（workspace 根） | 无 stdout | 写 managed SAB 后**转调宿主子进程** `sa build-exe <managed.sab>`，rc 透传 | SAB 管线 + 子进程 |
| `sab disasm <file.sab> [--out f]` |.sab 文件 | 反汇编文本 → stdout 或 --out 文件 | 可选写出文件 | **宿主 sci_bridge.disasmSabAlloc**（plugin_commands.zig:173；实现在 sci `plugin_bridge.zig`，非插件内代码） |
| `slab build\|workspace\|disasm` | 同 sab | 同 sab | 同 sab | 同 sab（args[1] 别名直达） |
| `build-exe [file]` |.sla 源文件 | 透传子进程输出 | 写 managed SAB 后转调 `sa build-exe`，rc 透传（plugin_commands.zig build-exe 分支） | SAB 管线（buildExeCompileOptions：load_reachable_imported_bodies_from_registry=true）+ 子进程 |
| `build-wasm\|build-js\|build-c [file]` | 同上 | 同上 | 同上，转调对应宿主主动词 | 同上 |
| `build-workspace` | 无源文件 | 同 build-exe | 同 build-exe | 同上 |
| `test [file] [--test-backend auto\|sab\|sa]` |.sla 源文件 | 透传子进程 `sa test` 输出（场地实测：`[PASS] t` + `test result: ok. …`） | 编译测试输入到 managed 路径，跑完删（delete_after）；无测试命中 filter 时直接写空结果 rc 0（plugin_compile_options.writeEmptyTestResult） | SAB 或 SA 测试输入编译 + 子进程 |

每个子命令另有 `-h/--help` 形态：用法 → stderr、rc 0（plugin_cli.zig
writeCommandHelp/commandUsage，逐命令静态文本）。

宿主侧注意（场地实测）：现头宿主对 `sa sla …` / `sa slab …` 形式在
**宿主层**向 stderr 追加一行弃用提示
（`note[SA-PLUGIN]: 'sa sla skills' is deprecated: …`），与插件实现无关，
两实现下相同 — 对拍时须从插件 stderr 中剥离或单独归属（§2）。

### 1.2 面 B 全集（sla_compile_handler）

签名与结构体见 ABI_MAP §5（与 plugin_handler.zig:4-25 一致；
sla-abi `handler.rs` 已逐字段镜像，布局测试 40/16/48 B 在案）。

- 返回码：0 成功；1 参数无效（error_name ∈ InvalidHandlerName /
InvalidHandlerSource / InvalidStateFields / InvalidStateFieldName /
InvalidStateFieldAddress / InvalidStateFieldType / InvalidBaseDir /
OutOfMemory，plugin_handler.zig:50-123 逐点）；2 编译失败
（error_name = Zig `@errorName(err)`）。
- 字段 ty 码：1=i1、2=i32、3=i64、4=f64、5=ptr（映射 boolean/i32/i64/f64/
void_type，handler_bridge.zig:51-59）；未知码 → InvalidStateFieldType。
- 编译管线（handler_bridge.zig:239-296，与命令面管线**不同构**）：
① 在源码前拼**写死的 extern 前导**（`render` / `sax_get_time` /
`sax_event_target_value_i64`，:245-255）；② source_expand.expand；
③ Parser.initWithDir(base_dir) 解析；④ handler 专用简版 import 展开
expandSlaImports（:207-226：仅递归.sla 文件、非.sla 导入保留为
import_decl 节点；与 plugin_import_expand 的 1,447 行全版是两套机器）；
⑤ Monomorphizer.monomorphize；⑥ TypeChecker.initWithOptions
（injected_scope_bindings = 状态字段（可写）+ ambient_bindings（const），
:265-283）；⑦ Codegen.initWithOptions（injected_address_bindings =
字段名→地址串，:285-289）走**文本 codegen**（不经 SAB）；
⑧ extractFunctionParts 按 `@sla__<name>(` 行切分 body/support，
filterSupportFunctions 只保留 `@sla__` 函数块并剔除注入的 `!<字段名>`
释放行（:88-149）。
- 副作用：除 base_dir 下相对.sla import 的**文件读取**外无写副作用；
无缓存、无 broker、无子进程。产物所有权：body/support 由
c_allocator 分配移交宿主，`sla_compile_handler_result_free`
（plugin_handler.zig:125-132）以同一分配器释放并清零结构体；
error_name 指向静态串不释放。
- 既有 Zig 侧用例（现成对拍种子）：plugin_handler.zig:134-169 的
C ABI 级测试（inc/count/i64，断言 body 含 `state+Counter_count` 与
`call @render()`）+ handler_bridge.zig:298-352 两例（注入状态读/写、
相对.sla import 展开走 support）。

### 1.3 Broker 交互点（仅面 A、仅 freshness 一条线）

- 唯一调用方：`plugin_freshness.zig`（319 行）。plugin.zig:229 在每次
handle_command 分发时把 Context 副本钉进全局 `plugin_freshness.host_ctx`
（进程级全局可变状态 — Rust 侧复刻需 static/thread_local 等价物，见 §4）。
- 触发点：plugin_compile.zig 的 compileSlaToSaStringWithOptions
（:761 查、:795 存，kind=sa_text）与 compileSlaFileToSabWithOptions
（:876 查、:927 存，kind=sab）。即命令面中只有 build / sab build /
build-exe 族 / test 输入编译经过 broker；**check / skills / stability /
init / help / disasm 不碰 broker，面 B 完全不碰**。
- 协议：op 5=cache_lookup、6=cache_store（plugin_api.zig:26-27）；
broker_available = broker_call 非空且 broker_abi_version ≥ 1
（plugin_api.zig:60-62）。lookup 为两段式：首呼 value_cap=0 探长度
（宿主回 BrokerStatus.insufficient_buffer=5 并给出 value_len —
sci plugins.zig:114-122 枚举实证），次呼取字节。key 材料 =
kind tag + gateKey（file 串 + options 摘要的 Wyhash，plugin_freshness
.zig:67-83、180-232）。closure blob 只含去重后的依赖**路径**清单，
依赖内容再哈希由宿主缓存服务负责（:53-59 注释口径）。
- 离线自理：broker 不可用时自动回落本地门（`.sla-cache/fresh/{stem}-{key}
.json/.bin` 清单+产物，:233-319），standalone 的 local_cli.zig 构造
无 broker 的 Context（local_cli.zig 末段）即走此路；环境变量
`SLA_NO_FRESHNESS_CACHE=1` 同时旁路两条路（:173-177）。
**结论：线束可自理** — 对拍时钉 `SLA_NO_FRESHNESS_CACHE=1` + 清
`.sla-cache` 即可把 broker 变量整段消去；broker 在场/不在场两形态
的等价性是独立课题（§4），不进首轮对拍。

---

## 2. 活体对拍的定义

### 2.1 一轮对拍比较什么

**面 A（宿主活体）**：同一宿主二进制（场地 sa 0.1.8 / sci e82d2865）、
同一命令 argv、同一沙盒目录布局（相对路径调用），轮换装载 Zig.so /
Rust.so（经独立 SA_PLUGINS_PATH/HOME 暂存目录 + sap.json 清单，
宿主发现逻辑 sci plugins.zig:1833-1847；SA_PLUGIN_DEV=1 免
permissions.lock 正式门 — 场地 env.sh 已置），逐项比较五元组：

1. **符号面门（第 0 门）**：`nm -D --defined-only` 全集逐行相等
（当前两边均为描述符双锚点，实测相等 — §4.1；任一侧多/少符号即红，
这正是 HND-C1 的机械判据）；
2. **rc**：进程退出码（= AbiStatus ok 时的 out_code）；
3. **stdout / stderr 逐字节**：先剥离宿主自产行（弃用提示 note[SA-PLUGIN]，
宿主两侧同产）后逐字节比；
4. **产物文件树**：沙盒内新增/改动文件的相对路径集 + 逐文件 sha256；
5. **stderr 中的插件诊断**：含在 3 内，不单独降级。

**面 B（函数活体，候四期形态）**：无宿主消费者，对拍台只能是
dlopen 直调或双侧打印驱动：同 (handler_name, handler_source, fields[],
options) 输入 → 比较 (返回码, error_name, body 逐字节, support 逐字节)
四元组，外加 result_free 后结构体清零的契约检查。种子语料 = §1.2 的
3 个 Zig 既有单测形态 + 增补形态（五种 ty 各一、未知 ty 码、空字段集、
null/空指针各参数、import 相对文件、编译失败件）。

### 2.2 逐命令确定性分级（场地实测 + 源码依据）

**A 级 — 离线确定性、可逐字节**（场地双跑 sha 一致已实测者标注 ★）：

- help / 各命令 `--help`：静态文本。★（rc=0 实测）
- skills --json ★：sha256 `efbe756e…` 双跑一致（静态表序列化，
plugin_skills.zig:64-79；Rust 侧 skills 静态表已逐字镜像，剩 JSON
写入器未接）。
- stability schema ★：sha256 `99bf7d04…`（静态常量直出）。
- stability verify：给定同一清单文件，报告字段序由写入器写死
（stability_metadata.zig:309-340），逐字节可比。
- init：文件树内容静态（仅包名 = 路径 basename 派生）；同相对调用下
stdout 逐字节可比（场地实测生成 proj/{sa.mod, src/main.sla,
.gitignore}）。
- skills（文本模式）：stdout + 两份 SKILL.md 均静态派生，孪生干净
cwd 下逐字节可比。
- 未知命令 / 参数错误族：stderr 静态模板 + rc。
- check（成功与失败）：成功行只嵌 file 入参串；失败诊断嵌 file 串与
行列，同布局同入参逐字节可比。前置：关 SLA_PROFILE（其阶段计时行
写 stderr，plugin_compile_options.zig slaProfileStage，默认关）。
- build（文本）：★ 平凡件双跑 sha 一致（`b6d269ba…`，含冷/暖
freshness 两轮）。条件见 B 级注记。

**B 级 — 规范化后可比 / 有条件逐字节**：

- build / sab build 的产物本体：对绝大多数程序逐字节可比，但有
**Zig 自身非确定源**两处在册：FB-020（return 清理名单按
StringHashMap 迭代序发射 — 文本路，REPORT §9.3 实证 `!b!a!c!d`）、
FB-016（逃逸闭包按 AutoHashMap 迭代序发射 — SAB 路）。命中这两类
形态的语料，Zig-vs-Zig 亦不保证逐字节 — 降级口径：对清理行集合做
排序规范化后比，或将该形态语料单列「序不敏感」组。不得因此把
Rust 的声明序输出判红（纪律同 COORDINATION §4.3）。
- stdout 成功行嵌 managed 路径：`.sla-cache/sab/{stem}-{Wyhash(file入参串)}.sab`
（场地实测 `a-6fc8eb46e50445bb`）— 同相对入参下逐字节同；入参串
形态（相对/绝对）不同即不同，对拍台钉相对入参即可，不必规范化。
- SA 文本产物按输出文件位置重写 @import 相对路径
（plugin_freshness.zig:215-219 注释 + plugin_output_paths.zig）—
孪生布局下自洽，跨布局不可比。

**C 级 — 本质不可逐字节（插件归属面之外）**：

- build-exe / build-wasm / build-js / build-c / build-workspace /
sab workspace / test：终产物与终输出由**宿主子进程**生产
（`sa build-exe` / `sa test` 等，resolveSaExecutable 见
host_paths.zig:69）。两插件共用同一宿主，终产物差异不归插件。
插件可归属面只有三项：① 中间产物（managed.sab / 测试输入文件）
字节；② 子进程 argv 构造（透传参数集、--test-backend/--filter 的
剥离规则，plugin_cli.zig:83-120 实证有专门单测）；③ rc 透传与插件
自产 stderr。对拍手法：PATH 上放记录型假 `sa` shim（记录 argv、
回放罐装 rc/stdout），把子进程面换成确定性桩后，①②③ 逐项可比；
真宿主全链只作冒烟（rc + 关键行），不作字节判据。test 的子进程
输出另含测试运行器文本（`[PASS] …` / `test result:` 行，场地实测
形态稳定），全链冒烟时按行集比对而非整流逐字节（运行器若在其他
模式输出耗时列，须先规范化 — 本轮未见，不预设）。

### 2.3 对拍台的环境钉法（两面共通）

同场地同宿主、孪生沙盒（同相对路径）、`SLA_NO_FRESHNESS_CACHE=1`、
跑前清 `.sla-cache` 与 `~/.cache/sa/frontend`（主线规程：前端缓存键
不含编译器版本）、关 SLA_PROFILE、固定 SA_PLUGIN_DEV=1、
HOME/SA_PLUGINS_PATH 指向各自暂存插件目录、语料文件 md5 对版。

---

## 3. 接线成本（估算，依据逐项标注）

估算口径：以已交付线的实测移植比为据 — parser 4,478 Zig 行 →
3,173 Rust 行（×0.71）、checker 6,374 → 约 3,648+（×0.6–0.75）、
lexer ×1.1。以下按 ×0.7 估 Rust 行数，**均为估算非实测**；拆分守
≤700 行/文件（COORDINATION §4.4）。

### 3.1 面 A0：无管线命令子集（help / skills / stability / init / 未知）

Zig 对应面：plugin_cli.zig 502（其中解析器 parseSlaCliOptions* 约 80 行
为全命令共用）+ plugin_skills.zig 153 + stability_metadata.zig 386
≈ 1,041 行。Rust 侧已备：skills 静态表（sla-plugin/src/lib.rs:24-47）、
HostStream 写封装（sla-abi descriptor.rs:117-131）、AbiStatus。

| 新模块（均落 sla-plugin） | 内容 | 估行数 |
|---|---|---|
| `dispatch.rs` | 替换 Failed 桩：argv 解码（NUL 串）、sla/slab 分发、AbiStatus/out_code 契约、HostStream → 写适配 | ~250 |
| `cli_options.rs` | parseSlaCliOptionsFrom 镜像（A0 只用 help/未知分支，全形态预留） | ~150 |
| `skills_cmd.rs` | writeSlaSkillsJson 逐字镜像 + 文本模式 + 两份 SKILL.md 生成（plugin_skills.zig:64-153） | ~200 |
| `stability.rs` | schema_json 常量嵌入 + validateManifestText + 报告双写器；**需手写 JSON 解析**（workspace 现零外部依赖，见 §4.3） | ~450（拆 2 文件） |
| `init_cmd.rs` | 脚手架三文件 + 独占创建语义 | ~120 |

合计 ≈ **1,170 Rust 行 / 5–6 文件**，无新 crate、无管线依赖。
oracle 侧改动：无（宿主即驱动）；对拍台为新脚本（§3.4）。

### 3.2 面 A1/A2：build / check / sab build（管线命令）

插件侧增量本身不大 — 前提是管线段齐备：

- 命令分支 + compile 主干接线（plugin_compile.zig 1,013 行中
runSlaFrontend + 两尾）→ sla-plugin `compile_wiring.rs` 等估 ~500 行；
- freshness 门（plugin_freshness.zig 319）→ 估 ~280 行（含 broker
两段式 + 本地清单双路 + host_ctx 全局钉入）；
- sab 路径/写盘（plugin_sab_paths.zig 220）→ 估 ~160 行；
- workspace 解析（workspace.zig 705，build-workspace/sab workspace 用）
→ 估 ~450 行；
- **前置缺段（非本线体量、决定排期）**：合同解析+导入展开编排
（INT-2 线辖区，COORDINATION §8 D-A2：sla-parser 合同族 +
sla-plugin 接线层）；source_expand（450 行 Zig，Rust 无对应模块 —
crates 全扫实证仅注释提及）；monomorphizer（1,905 行 Zig，
**Rust 无对应模块**，checker 注释自陈「instantiation is the
monomorphizer's job」而该 job 无人持有）；SAB D 组表面
（sab build 需之，SAB-C 线在途）；sab disasm 需 sci 反汇编器
（sci sab.zig 反汇编段，未测行数，插件侧无此代码 — Rust 化等于
新移植一条宿主段，或改走子进程转调，但 Zig 是进程内直调、
转调会改变错误文本面，属有意分歧候裁）。

### 3.3 面 B：sla_compile_handler

| 件 | 状态/估算 |
|---|---|
| ABI 类型 | **已备**（sla-abi handler.rs 80 行，布局测试在案） |
| 导出锚点 | sla-plugin 加 `#[no_mangle] sla_compile_handler` + `_result_free` 两函数 + 所有权（Box 泄漏/回收，无需 libc 依赖）≈ 40 行 — **但须等 HND-C1 裁定后才许接**（§4.1：现与出货符号面保持一致优先） |
| ABI 壳（参数校验/error_name 映射） | plugin_handler.zig 壳 ≈ 120 行 → 估 ~130 行 `handler_abi.rs` |
| bridge 编排（前导拼源、extract/filter 字符串手术） | handler_bridge.zig 非管线段 ≈ 200 行 → 估 ~200 行 |
| checker 注入绑定 API | Zig TypeChecker.initWithOptions 的 injected_scope_bindings — Rust Checker 无此面，估 +100~150 行（checker.rs/scope.rs，**CHK3 辖区相邻，须协调**） |
| codegen-text 注入地址 API | Zig Codegen 的 injected_address_bindings（标识符直接解析为地址串 load/store）— Rust codegen-text 只有 addressable_bindings（`&name` 收集，不同机器），估 +150~250 行 |
| source_expand 移植 | 450 Zig 行 → 估 ~350 行（**无主缺段**） |
| monomorphizer 移植 | 1,905 Zig 行 → 估 ~1,300 行（**无主缺段、本面最大头**） |
| handler 专用 import 展开 | handler_bridge.zig:171-226 ≈ 90 行（与全版不同构，须单独镜像，**且其错误吞噬见 HND-C2**） |

最小子集变通（登记备选，不推荐为首发）：对无泛型、无 tuple 模板的
handler 子集跳过 mono/expand 两段（Zig mono 对无模板程序近恒等 —
**此点未实测，属推断**），命中泛型/模板形态**响亮拒绝**（仿 SAB
线 Unsupported 口径）。可省 1,650 行估算，但 checker/codegen 注入
API 两件仍是硬前置，且「近恒等」须先以 Zig 侧探针证实才许立项。

### 3.4 oracle / 对拍台改动面

- 面 A：宿主即驱动，oracle bin 不改。新增对拍脚本 + 语料清单 —
**注意辖区**：`scripts/**` 与 `results/p3/**` 在三期线位表中归 ORC 线，
HND 对拍台脚本落点须协调线裁（自建 `scripts/handler-parity/` 子目录
或并入 ORC 协议），不得径直占 ORC 路径。
- 面 B（四期）：sla-oracle 加打印驱动模式（读用例 → 调本侧 bridge →
打印四元组），或独立 dlopen 驱动（workspace 零外部依赖下需手写
dlsym 声明 ≈ 60 行，可行）。Zig 侧对应驱动可由其既有单测改造成
打印形态，成本在 Zig 仓侧、不归本实验线。
- 语料：面 A 复用 fixtures 01–07 + e2e_a/b/c + 新 A0 命令矩阵
（每命令 × 正常/帮助/错误三形态 ≈ 20 例）；面 B 种子见 §2.1。

---

## 4. 风险与前置

### 4.1 HND-C1：第二 ABI 在现头出货插件中不存在

实测（2026-11-07，场地三现头 sla `6002b386`，三种独立手法互证）：

1. `nm -D --defined-only` 于场地已装插件
`/root/.local/share/sa_plugins/installed/sla/current/libsla.so` 与
构建产物 `zig-out/lib/libsla.so`：**均只有 2 个定义符号**
（`D saasm_plugin_descriptor_v1`、`T saasm_plugin_descriptor_v1_fn`）；
2. `strings -a libsla.so | grep -c sla_compile_handler` = **0**
（符号名在二进制中零出现）；
3. python3 ctypes dlopen 两侧.so 查三符号：Zig 侧
`sla_compile_handler=False`、`sla_compile_handler_result_free=False`；
Rust 侧 release.so 同为 False（两边当前符号面恰好一致）。

同时：`grep -rn sla_compile_handler sci/src/`（场地现头）**零命中**，
`strings sa | grep -c sla_compile_handler` = 0 — sci 宿主内不存在
按名查找该符号的消费者；plugin_handler.zig 在 sla 仓的 git 史只有
一笔 `e68634c6 part1`，其唯二消费者是自带单测。

与既有文档的矛盾：docs/ABI_MAP.md §1 表格称该两符号为 libsla.so
「实测 nm -D」的 T 符号 — 与现头实测不符（可能测于他版构建/测试
二进制，或登记有误，本线不裁原因，只登记矛盾）。REPORT §7.2 的
口径「第二 ABI 按设计未导出」描述的是 **Rust 侧**；Zig 侧的真实
状态此前无人登记。

候选机制（**推断、未证**）：plugin.zig:25-26 仅以
`pub const sla_compile_handler = plugin_handler.sla_compile_handler;`
再出，lib 根无其他引用；Zig 惰性分析下 plugin_handler 的 export
声明未被发射。证实需 Zig 侧探针（根部 comptime 引用后重编 nm 对照），
不在本线辖区。

影响（改变评估结论）：① 面 B「活体对拍」当前没有可对的活体 —
Rust 若先行导出该符号，符号面反而**多于**参照（第 0 门先红）；
② 顺序必须倒置：先由主线裁定「Zig 补导出（修插件）vs ABI_MAP
更正（该 ABI 属未出货设计）」，参照成立后 Rust 才接线；
③ 若裁定为补导出，其修复按主线纪律须带回归（nm 门或宿主加载
探针）。本项已在本线回主会话消息中单独标出；文档内编号候选
登记见 §6（候协调线收编为 FB-035）。

### 4.2 与 INT-2 合同段的耦合

- 面 A0：**零耦合**（不读.sai、不展开导入）。
- 面 A1（build/check）：**强耦合**。check 分支与 build 主干都调
loadImportedContractsFromResolvedImports（plugin_commands.zig
check 分支、plugin_compile.zig runSlaFrontend）— INT-2 未落地前，
含 std 导入的程序（即全部 5 个 fixtures，REPORT §9.3 阻挡清单）
在 Rust 侧 check 段即死（rc=5），命令面对拍只能覆盖无导入程序。
- **辖区冲突（排期硬约束）**：三期线位表把 `crates/sla-plugin/**`
划归 INT-2 线（其接线层落点，D-A2）。面 A 的 dispatch 接线同在
sla-plugin（lib.rs 桩即 INT-2 辖区文件）。A0 切片若三期开工，
必须由协调线切分 sla-plugin 内文件辖区（如 INT-2 持
contract/expand 接线模块、A0 持 commands 模块，lib.rs 改动串行
经协调线），**不能作为与 INT-2 完全独立的并行线径直开工**。
- 面 B：与 INT-2 **无耦合**（handler 管线不加载合同；其 import
展开是 handler 专用简版）— 但被 mono/expand 两段无主缺段挡住，
该两段亦不在 SAB-C / CHK3 辖区内，三期无人顺带解锁。

### 4.3 其他耦合与风险

- **零外部依赖纪律**：workspace 全部 crate 只有 path 依赖
（各 Cargo.toml 实查）。stability verify 需 JSON 解析、面 B 驱动
需 dlopen — 均可手写（Zig 侧 skills JSON 亦手写），但若要引
serde_json/libloading，属 Cargo.toml 依赖边变更，按三期操作
注记**只归协调线**，且 Kaggle 场地离线拉取新 crate 的可行性
未验 — 立项前先验。
- **上下文布局耦合**：Context 是双侧手工镜像的普通 struct（D-C1，
实测 72 B 重排布局），宿主 sci 升级改 Context 时两实现同碎；
对拍结论只对钉死的宿主版本（sa 0.1.8 / sci e82d2865）成立，
换宿主须重跑 ABI 探针（docs/probes/abi_probe）。
- **全局状态**：plugin_freshness.host_ctx 为进程全局（§1.3）；
宿主若并发分发命令，Zig 侧即有竞态面 — Rust 复刻时按原样镜像
并登记，不在本线裁改。
- **权限/清单耦合**：Rust.so 进真宿主须配 sap.json（name=sla、
abi.plugin=1）+ 权限确认；dev 模式（SA_PLUGIN_DEV=1）免正式
permissions.lock 门，正式模式装载路径未验 — 对拍台按 dev 模式
口径设计，正式模式列未验项。
- **SAB-C / CHK3 相邻面**：sab build 对拍等 SAB D 组（SAB-C 线
INT-2 后接 D 组）；check 对拍的边角形态受 CHK3 剩余 3 项影响
（关联调用推导、match 守卫、plain-if integer 条件），首轮语料
避开即可，不构成阻塞。
- **计时线（ORC）**：无直接耦合；但 A0 落地后 `sa sla skills` 等
命令的端到端计时口径多一条真宿主路径，ORC 协议若纳入须另注
「经真宿主」与现轻线束口径分开（COORDINATION §8 计时证据口径）。

---

## 5. 结论建议

### 5.1 总建议：分拆处置

1. **面 B（sla_compile_handler 活体对拍）：不纳入三期，候四期。**
理由：HND-C1 使参照面当前不成立（§4.1）；mono/expand 两段无主
缺段（§3.3，估 ~1,650 Rust 行的前置工程，不应由对拍线顺带
吞下）；checker/codegen 注入 API 两件硬前置未排期。四期立项
前置条件（齐备才开）：① HND-C1 裁定落地（Zig 补导出或
ABI_MAP 更正，二选一有实证）；② mono/expand 归属线完工或
「无泛型子集 + 响亮拒绝」变通经探针证实并裁定；
③ §2.1 面 B 四元组对拍台与种子语料就绪。
2. **面 A 最小切片 A0：建议纳入三期**（独立小线或并入 INT-2 线的
sla-plugin 辖区切分，§4.2）。理由：成本 ~1,170 行估算、无管线
耦合、全 A 级确定性、且是 REPORT §3.3「Rust cdylib 挂进真 sa
跑」这一集成第一站的真实兑现点 — 宿主加载/校验/分发路径
（loadDescriptor → validateDescriptor → handle_command）整段
第一次被 Rust.so 走通，分歧发现面（Context 布局、HostStream
契约、argv 约定）密度与 ABI_MAP 摸底直接对应。
3. **面 A1（build/check）**：不单独立线 — 随 INT-2 落地后在
三期尾段以「INT-2 解锁的 fixtures 子集」做命令面跟随对拍
（§5.2 第 2 批），归 INT-2 线交付注记或协调线收尾件。
**A2/A3（sab build/disasm、子进程族）候四期**（等 SAB D 组与
disasm 归属裁定）。

### 5.2 若纳入：最小切片定义（A0 + 跟随批）

**第 1 批（A0，命令 × 形态矩阵，每例比 §2.1 五元组）：**

| 序 | 用例 | 判据 |
|---|---|---|
| 1 | `sa sla help`、`sa sla`（无子命令）、argv<2 形态 | stderr 逐字节 + rc（0 / 1 / AbiStatus unknown_command 经宿主诊断形态观察） |
| 2 | `sa sla skills --json` | stdout 逐字节（已知金值 sha `efbe756e…`） |
| 3 | `sa sla skills`（文本，孪生干净 cwd） | stdout + 两份 SKILL.md 逐字节 |
| 4 | `sa sla stability schema` | stdout 逐字节（金值 sha `99bf7d04…`） |
| 5 | `sa sla stability verify` × {合法清单 JSON 报告、合法清单文本报告、非法清单、缺文件} | stdout/stderr 逐字节 + rc（0/1/1） |
| 6 | `sa sla init proj` + 重复 init（已存在） | 文件树 sha + stdout 逐字节；重复例 rc 1 + stderr |
| 7 | `sa sla nosuchcmd`、各命令 `--help` 抽 3 例 | stderr 逐字节 + rc |
| 8 | 第 0 门 | nm 符号面逐行相等（贯穿全批） |

**第 2 批（跟随批，INT-2 落地后解锁，命令面跟随对拍）：**
`check` / `build` 于 e2e_b、e2e_c（无 std 依赖、oracle 已逐字节
全等两件）先行 → fixtures 01–07（INT-2 解锁后）→ B 级规范化组
（FB-020 形态单列）。判据同 §2.1，产物.sa 逐字节。

**明确不在切片内**：sab build/disasm、build-exe 族、test、
build-workspace、面 B 全部 — 理由见 §3.2/§5.1，候四期逐项立项。

---

## 6. 缺陷候选登记（只登记、不落 FB 单；候协调线收编，预留 FB-035）

| 编号 | 级别 | 内容 | 证据 |
|---|---|---|---|
| HND-C1 | **重大（改变结论）** | 出货 libsla.so 未导出 sla_compile_handler / sla_compile_handler_result_free，与 ABI_MAP §1「实测」表矛盾；sci 宿主内无消费者 | §4.1 三手法实测（场地现头 6002b386）；plugin.zig:25-26、plugin_handler.zig:50/125、git 史 `e68634c6 part1` |
| HND-C2 | 小（诊断保真） | handler_bridge 的 import 展开把「读文件失败 / expand 失败 / parse 失败」统一吞成 InvalidHandlerOutput（handler_bridge.zig:195-198 的 catch 链），宿主侧 error_name 读作 InvalidHandlerOutput、与真实因（IO/语法）脱节；rc=2 分类下诊断误导 | handler_bridge.zig:171-205 源码 |
| HND-C3 | 观察（表面限制未声明） | compileHandlerWithSupport 写死 extern 前导三符号（render / sax_get_time / sax_event_target_value_i64，handler_bridge.zig:245-255）：handler 源码调用任何其他宿主函数必在检查段失败，该限制在 ABI_MAP §5 与 handler.rs 类型面均未声明 | handler_bridge.zig:239-296 源码 |
| HND-C4 | 观察（双路漂移面） | check 命令在 plugin_commands.zig 内联复刻前端管线而非共用 plugin_compile 主干（parse_test_bodies=false 写死、include_all_imported_decls=true、无 freshness），build 与 check 两路存在长期漂移面；本轮未做双路差分实测，仅登记结构事实 | plugin_commands.zig check 分支 vs plugin_compile.zig:591-807 |

已查明**非**缺陷、不登记（省后续线重查）：plugin_freshness 的
broker 两段式 lookup 中首呼容忍 status 5 — 5 是 BrokerStatus
.insufficient_buffer（sci plugins.zig:114-122），为协议正常形态。
plugin_handler.zig 的 abiSlice（len==0 → 空切片、null 不查）已在
REPORT §2 以 D-C2 在册，不重复登记。

---

## 附：证据索引

- 场地探针（2026-11-07，场地三 2202，/tmp/hndprobe 沙盒）：
skills --json sha `efbe756e11fc…` 双跑一致；stability schema sha
`99bf7d0459f3…`；help rc=0；未知命令 rc=1 + 原文；check 成功行
原文；build 平凡件冷/暖 sha `b6d269ba4735…` 一致；sab build
managed 路径 `.sla-cache/sab/a-6fc8eb46e50445bb.sab`；init 文件树
三件；test 子进程输出 `[PASS] t` + `test result: ok. 1 passed`；
宿主弃用提示 note[SA-PLUGIN] 实样。直调 sa 二进制未 source env.sh
时报 libz3.so.4 缺失（场地工具链须 source
/kaggle/working/content/tools/env.sh，内含 SA_PLUGIN_DEV=1）—
对拍台脚本须固化此步。
- 符号实测：本机 local 树 zig-out/installed 两份 libsla.so 与场地
现头两份，四份均只导出描述符双锚点；Rust release.so 同。
- 本地参照树与现头的版本差：local-sa 头 `3de91932`（2026-10-26）
≠ 现头 `6002b386`；本评估的 handler/commands 面行号经场地抽查
对版（§头注），plugin_compile/freshness/sab_paths 等未逐行对版的
文件行号以本地树为准、结论不依赖其行级精度。
