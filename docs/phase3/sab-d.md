# 三期 SAB-D 线交付报告 — D 组（std 表面基础设施）

场地：Kaggle 场地三（2202）；本地主本先行、自家路径逐路径
擦除→tar 管道→md5 双确认（每轮全等）。基线 sab 44/0、
workspace 226/0 未倒退：本线终验 sab **73 passed / 0 failed**
（lib 52 + driver_e2e 11 + driver_e2e_d 7 + golden 3）。

## 1. 模块落地清单（D1–D5）

| 模块 | 文件 / 行数 | 对应 Zig 行段 | 内容 |
|---|---|---|---|
| D1 | `src/surface_load.rs` / 672 | 1273–1589 | `readStdSurfaceFile` 四级回退链（SLA_STD_DIR → ~/projects → self-exe 三候选 → cwd 四候选含 vscode 硬编码）、`resolveSaStdRoot`（烘焙目录优先，与主线实证一致；其后 SA_STD_DIR → SCI_ROOT → home 两根 → cwd 六候选；四必需文件有效性；全败 FileNotFound 响亮）、`cachedSaStdRoot` 语义由 state 字段承载、规则解析全家（八类 kind、function/constructor 的 member/type 交换序、deps=/panic=/consume_value 选项、fallible 必须带 panic=）。文件缺失静默零规则**原样镜像**，回退链全程写入 `SurfaceLoadLog`（SAB-D02 方针，登记 SAB-D28 / FB-026） |
| D2 | `src/surface_load.rs` 同上 + `src/layout.rs` / 593 | 1590–1706, 1776–1782 | `primType`（File→u64、abiPassesAsPointer→ptr、余者响亮）、`paramPrimType`/`storagePrimType`（bool→u8）/`addressableSlotPrimType`（infer→ptr）、`typeSize`/`alignOffset` 转 lowering、`tuple/array` 布局（详细版以 `LayoutOutcome` 区分「无此字段」与「字段类型不可映射」，SAB-D10 口径；折叠版保留）、`structSize`/`structFieldLayout`（含 union/数组内联存储）、`structDeclForType`（structs→别名缓存→本地名回退 + 名字键缓存；CheckInfo 单表兼两槽，见 §5）、`fieldLayout`/`fieldType`、`typeBaseName`/`firstGenericArg`、`elementSlotSize`/`elementLoadType` |
| D3 | `src/copy_facts.rs` / 436 | 1783–1883 | `typeHasCopyDerive`（base 事实 + struct gate + 字段递归 + `copy_value_cache`；三处 `catch {}` 缓存写按 SAB-D05 登记——Rust insert 无失败通道可吞，不伪造）、`typeIsCopyStruct`、`typeIsCopyValue`（含 Chan 句柄规则）、`typeIsShallowCopyCallArgValue`（深度 >8 硬假、集合 depth>0、智能指针/std-owner 门）。lowering 未移植的 base 事实（typeHasCopyDeriveBase 等）按现头逐行本地移植、标注待归位 lowering（D-C3 同 pattern） |
| D4 | `src/surface_query.rs` / 533 | 1884–1997 + ensure 家族 2099–2101/6108–6136/13203–13229 | `findStdSurfaceRule`、`stdSurfaceRuleHasArg`、`StdSurfaceValues`、`stdSurfaceArgText`（含 sci `primTypeName` 口径）、`stdSurfaceArgMustBeLiteral`、`primTypeMacroSuffix`、`stdSurfaceMacroName`（`{elem_ty}` 全替换）、`emitStdSurfaceRule`（deps 先行、BTreeSet 直调特例、通用尾经 `SurfaceEmitter` trait——默认实现 F 组响亮拒绝）、`StdDepEnsurer` trait + `PendingStdDeps`（Zig 缺失判定与入队序逐字；append 时点分歧登记 SAB-D26）、println/debug 的 deps 清单逐字 |
| D5 | `src/surface_preload.rs` / 447 + `src/surface_scan.rs` / 353 | 1998–2317 + 2419 | preload 全 walker（Zig 臂集逐字：call 臂的 thread_spawn → println → debug → bare vec → bare pop → associated / function+method+fallible / constructor 的检查序、index_assign 先行检查、identifier constructor、index/index_address 双查；Zig 不递归的 switch/match/slice/try/await 同样不递归）、future/thread 扫描全 walker（await/async/task/executor 计划名表本地移植）、`stackAllocSize`（常量折叠走 lowering `int_constant_expr_value`、负数响亮、缺省 16）。两文件为一个 D5 单元，因 700 行纪律拆分 |

新增测试文件：`tests/driver_e2e_d.rs` / 292（7 个 e2e，
见 §3/§4）。既有文件改动：`src/driver.rs`（§2）、
`src/state.rs`（ProgramState 加 `surface_load_log` 字段）、
`src/lib.rs`（§6）。

## 2. driver 销账表（SAB-D19 余项中属 D 组的 B4 调用点）

| 原响亮点 | 处置 |
|---|---|
| step 2：表面 load + preload 整段未接线（原以注释占位、ImportDecl 空转） | **销账**：D1 真实加载（回退链 + 解析 + 日志入 state）、D5 真实预载（deps 入 `pending_std_deps`，D4 ensurer）；Zig 的 preload 错误包装 trace 后重抛，端口错误自带原因（SAB-D15）直接传播 |
| step 3：future 门为子集扫描（仅 spawn/join/select 等名字） | **换 D5 全扫描**：`program_uses_future_task_runtime` 逐字口径（async fn / await / task·executor 计划）；helper 发射仍 H 组响亮拒绝，归属与行段更新为 Zig 614–617 |
| gen_call：println / str_eq / associated·method 调用 | **保持响亮（死点下移）**：step 2 已真实工作后，这些调用仍死于 gen_call 表面展开（D 组归属不变）——展开本体是 F 组片段机器（`emitStdMacroFragment*`），D4 已把规则侧（查询/参数渲染/宏名展开/deps）全部备好并以 trait 隔离 |
| D2 primType（driver 内子集版） | 保留（B4 子集适配，覆盖 Pointer/Borrow/FnPtr/Closure→ptr 的既有口径）；D2 规范版在 `layout.rs`，两者差异仅在 Closure 一形（Zig 本尊亦响亮），待 E/G 组接线时统一 |
| 保持响亮（非 D 组） | F/G/H/I 全部调用点原样，措辞与组别未动 |

## 3. 测试数字

- 本线新增单测 22 个（D1 6 + D2 4 + D3 3 + D4 5 + D5 3 +
  scan 2；含真实 meta 摘录解析、回退链落点/优先级、
  sa_std 根有效性、布局 miss 双分、Copy 缓存三态、deps
  去重序、BTreeSet 特例渲染、preload 发现序、stackAlloc
  四形），期望值均出自 Zig 源码语义与现头 meta 原文。
- driver_e2e_d 新增 7 个（§4）。
- sab 终验 **73/0**（基线 44/0，+29）；workspace 全量：
  除 sla-plugin `contracts_pipeline` 1 红外全绿（见 §5
  备考——该红与本线无关，属跨线同步 skew）。

## 4. 端到端对照（场地三现头真插件）

真插件入口为插件仓 `zig-out/bin/sla sab build`（host `sa`
需 LD_LIBRARY_PATH 指向场地 llvm14 工具目录）：

| 探针 | 真插件产物 | 本端口 |
|---|---|---|
| `println("hello")` + return 0 | **1002 B，sha256 `f7ef75112407c56bf41a208e4558f673fb0f09674b51f6f3f2ffc245b244de54`** | 死于 gen_call 的 println 表面展开（group D；`driver_e2e_d::d_println_dies_at_gen_call_surface_expansion` 钉死）——展开本体 = F 组 `emitStdMacroFragment` + G 组 format 机器，未落地前无字节可对 |
| `let v = Vec::new(); return 0;` | **798 B，sha256 `131c90aa7f4e290c4aafaaf89206addc1e948eac263821717ba23b5a66013e03`** | preload 阶段已真实发现 associated 规则并入队 `sa_vec_new`（e2e 钉死入队序）；发射死于 gen_call associated 展开（group D → F 组片段） |
| 无表面程序（main 返回 42） | —（与真插件的既有差异仅 SAB-D19② 的 L_ENTRY 省略面，非 D 组） | 规则加载开/关两种 roots 下输出**逐字节相等**（`d_step2_loads_rules_and_output_is_emission_neutral`）——D 接线发射中性实证 |

规则文件全量核对：现头 `std_surface.sla_meta` 共 **57 条**
规则（associated 16 / method 23 / function 5 / constructor 4 /
fallible_method 1 / index 3 / index_address 3 / index_assign 2），
全部 kind 与选项形态在 D1 解析器覆盖内（场地 grep 计数 +
摘录解析单测双证）。

**死点登记（精确）**：含 std 表面调用的程序在本端口的死点
统一为——step 5 `gen_call` 的表面展开点（println /
associated / method / function 规则调用），其后依次是
F 组 `emitStdMacroFragmentWithLiteralArgs`（D4 已以
`LoudSurfaceEmitter` 响亮占位）与 `appendStdDepsNow` 的解码
模块 append（SAB-D26 的延后 append）。E–I 组其余调用点未动。

## 5. 新分歧 / 反馈清单

- 分歧新增 3 条（sab.md）：SAB-D26（b，ensureStdDeps 排队
  分支无条件采用、append 时点延后至 F 组）、SAB-D27（b，
  浅拷贝枚举臂因 CheckInfo 无枚举表不可达、触发面更窄）、
  SAB-D28（c，表面规则文件缺失静默零规则，D02 家族新实例）。
  sab.md 总账：(a) 2 / (b) 12 / (c) 14 = **28 条**。
- D2 结构注记：Zig `structDeclForType` 的四级查找中，
  `structs` 与 `alias_struct_cache` 在 Rust CheckInfo 上
  是同一张表（第二级探测恒与第一级同果），本地名回退两级
  同理——已写入 layout.rs 文档，不单列分歧号（与 SAB-D24
  的 trait 事实面同族，归 INT-2 接线面收敛）。
- 反馈单 1 张：**FB-026**（号段 026–028 内）——零规则静默
  继续（机制 + 与 resolveSaStdRoot 响亮口径不一致的对照 +
  建议形态）。FB-027/028 未用，退回号段。

## 6. lib.rs 改动清单（逐条）

- 新增 `pub mod copy_facts;`、`pub mod layout;`、
  `pub mod surface_load;`、`pub mod surface_preload;`、
  `pub mod surface_query;`、`pub mod surface_scan;`
  （按字母序插入模块表）。
- 顶部文档新增 Phase 3 (SAB-D) 段落（模块与行段对照、
  step 2 接线与 SAB-D26 说明）。
- 新增 `generate_from_program_with_roots` 入口（显式 D1
  roots 注入，driver 同名函数转发）。其余声明未动；
  Cargo.toml 依赖边**未动**。

## 7. 剩余项 / 备考

- **F 组前置已备齐**：片段机器落地时只需实现
  `SurfaceEmitter` + `flushPendingStdDeps` 消费端，
  D 组无需再动。SAB-D27 待 CheckInfo 枚举面接线后销账。
- **烘焙目录注入**：`sa_std_source_dir` 在 Rust 侧经
  `SLA_SAB_BAKED_SA_STD_DIR` 环境槽注入（未设 = 与 Zig
  无效烘焙根同行为：试、拒、续链），正式 build.rs 注入
  归协调线接线面（SAB-C §7 注①的遗留项，本线未越界动
  sla-plugin）。
- **workspace 1 红备考（非本线）**：sla-plugin
  `contracts_pipeline::println_remains_blocked_by_missing_checker_builtin_arm`
  在场地红——该测试只走 parser→合同加载→checker（不经
  SAB），其场地文件是旧版（期望 println 无 checker 臂而
  失败），而本地主本同名测试已更新为
  `println_resolves_through_checker_builtin_arm` 期望成功
  （INT2-1 已闭合）：场地该文件未随 INT-2/CHK3 线同步，
  属跨线 skew，备协调线对账（本线未动其辖区路径）。
- 过程注记：本线中途本机 Python 写文件触发已知写拦截
  伪影（AGENTS.md 在册），`surface_query.rs` 一度被截为
  0 字节——当轮以 muse.write 全量重写并经场地 md5 双确认
  与全量测试复原，无残损。
