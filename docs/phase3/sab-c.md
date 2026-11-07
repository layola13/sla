# 三期 SAB-C 线交付报告 — C 组（预扫与符号）

场地：Kaggle 场地三（2202）；本地主本先行、自家路径逐路径
擦除→tar 管道→md5 清单双确认（每轮 0 差异）。基线 sab 17/17
未倒退：本线终验 sab **44 passed / 0 failed**（lib 30 +
driver_e2e 11 + golden 3）。

## 1. 模块落地清单

| 模块 | 文件 / 行数 | 对应 Zig 行段 | 内容 |
|---|---|---|---|
| C1 | `src/prescan_consts.rs` / 657 | 633–854 | `collectGlobalScalarConsts` 全量（三相：直字面量→别名不动点→二元折叠不动点，内层别名环逐字镜像）、`scalarConstantNodeFor`、`foldTopLevelBinaryConst`（int/float/bool 全臂；溢出/除零/越界移位均放弃折叠）、`collectGlobalArrayConsts` + `emitGlobalArrayConst`（hex const 发射、stride 零填充、非字面量值静默不登记——原样镜像）、`storagePrimType`/`arrayStride` 适配 |
| C2 | `src/prescan_bindings.rs` / 582 | 855–934 + 2954–3056 | `collectAssignedBindingsInNode` 全 walker（rootIdentifier 记名、含 impl/overload 方法体）、`collectBorrowedBindingsInNode` 全 walker（每函数入口扫描，见 §3 更正）、`prepareMultiLetBindings`（转 sla-lowering 既有实现）、`bindingNeedsScalarReassignSlot` / `bindingNeedsCopyScalarReuseSlot`、`recordReg`/`finishFunctionRegs` 的完整账本 `RegLedger`（含 released 集合的记录复活语义） |
| C3 | `src/symbols.rs` / 541 | 935–1102 | `intern`/`internStable`、`bindingReg`、`loweredFuncSymbol`（含 `sa_` 前缀与 extern_funcs 两条后置规则——driver 旧版缺失、本轮补齐）、`hasConcreteFunctionSymbol`、`mangleMethodName`/`mangleTraitMethodName`（转 lowering）、`concreteTypeName`/`dynTraitName`、`vtableName`、`appendTraitVTableEntries`（超 trait 先行、decl-only 跳槽、逐条 intern 顺序）、`emitTraitVTableDecl`、`fnPtrVTableName`/`ensureFunctionPointerVTable`/`appendVTableConst`（intern 顺序与 trait 路不同，逐字镜像） |

既有文件改动：`src/emit.rs`（+18 行：`append_const_decl`、
`finish_function_regs` 两个加法钩子，无既有行为变更）、
`src/driver.rs`（见 §2）、`src/driver_gen.rs`（gen_call 的
lowered 名改走 C3 `lowered_func_symbol`）、`src/lib.rs`
（见 §5）、`tests/driver_e2e.rs`（+4 测试，见 §4）。

## 2. driver 销账表（SAB-D19 中属 C 组的响亮报错点）

| 原响亮点 | 处置 |
|---|---|
| prescan：全局 const 别名折叠报错（C1 633-720） | **销账**：`collect_global_scalar_consts` 全量接线，别名/二元折叠均真实工作（e2e 钉死，§4） |
| prescan：全局数组/非标量 const 报错（C1 721-854） | **销账**：checker 记录数组类型者真实发射 hex const；无类型事实者按 Zig `orelse continue` 静默跳过；值非字面量者按 Zig 静默不登记 |
| prescan：assigned 收集仅标识符目标的残缺 walker | **销账**：换 C2 全 walker（根标识符、impl/overload、let-else/destructure、match/for/closure 等全臂） |
| step 4：trait vtable decl 报错（原挂 I 组 550-556） | **销账（C3 部分）**：vtable const 真实发射（程序本地 trait）；impl 方法体本身仍 I 组响亮拒绝（step 5 不变）。导入 trait 的触发面差异登记 SAB-D24 |
| step 5：decl-only extern 无条件发射 | **修正接线**：C3 `hasConcreteFunctionSymbol` 守卫——有同名具体定义时原型不发射（Zig generate 原行为；e2e 逐字节钉死） |
| gen_call：lowered 名仅 main 例外 | **修正接线**：改走 C3 全规则（extern/no-mangle/extern_funcs/`sa_` 前缀） |
| binding_reg / lowered_symbol（driver 内旧实现） | 改为 C3 模块委托，语义为旧版超集（旧测试全绿维持） |
| borrowed bindings（driver 未收集） | **接线**：每函数/测试入口按 Zig 6212/6477 同一点收集（见 §3）；导入宏 plan 缺口登记 SAB-D23 |
| 保持响亮（非 C 组） | D 组表面（println/str_eq/字符串物化）、F/G/H/I 全部调用点原样响亮，措辞与组别归属未动 |

## 3. 重要更正（本线初版误判，已当轮纠正）

初版曾据**被 head 截断的 grep 输出**登记「borrowed walker
在 Zig 无调用点（死代码）」。全量复查证伪：它在 8 个函数
入口路径被调用（6060/6117/6152/6212/6477/7690/7909/7950），
是每函数扫描而非预扫。已按真实形态接线并把教训写进
SAB-D23。grep 摸底不许截断下结论——与协调线「计数对上≠
内容对上」同族。

## 4. 测试数字与真产物对照

- 本线新增单测 23 个（C1 8 + C2 6 + C3 9，均在模块内），
  期望值出自 Zig 源码语义（折叠 -43 链与官方语料
  test_unit_toplevel_const_binary_fold.sla 同形）。
- driver_e2e 新增 4：const 折叠端到端（与直写 42 的程序
  **逐字节相等**）、数组 const 物化（含未类型化对照组）、
  extern 原型抑制（与无原型程序逐字节相等）、真产物段级
  对拍（下条）。
- **真产物对照（场地三现头插件 `sa sla sab build`）**：
  - arr 探针（`const TABLE: [i32; 3] = [1, 2, 16909060]`）：
    真产物 225 B / sha256 `a114ec0329fa…`；段级比对——签名段
    19 B 同、**常量段 21 B 逐字节同**、池与指令段差异仅
    `L_ENTRY`（池 +8 B、指令 +20 B，SAB-D19② 既登记项）。
    端口输出 197 B / sha256 `0186262801…` 已钉进 e2e。
  - vt 探针（trait Named + impl for Sprite）：真产物池含
    `VT_Sprite_Named` 与字面量 `vtable { name =
    @sla__Sprite__Named_name }`，与 C3 发射逐字一致；intern
    序（lowered 名先于 vtable 名）亦同。
  - fold 探针：真插件编译通过；折叠语义值由单测钉死。
  - hex 反斜杠：真产物字节列 [104,101,120,58,92,120,48,49]
    证明显式文本为**单**反斜杠（sci managedPrint 折叠源码
    格式串的双写）；端口初版双写错误被此对照抓出并修正
    （本移植缺陷，登记于 sab.md 已核查排除节）。
  - shl 溢出：真产物 `3 << 62` 折为 -4611686018427387904，
    与端口 wrapping_shl 一致（已核查排除）。
- workspace 全量（场地三终验）：本线 sab 44/0；其余 crate
  中 checker 出现 tests_phase3b 14 红（104 过）——CHK3 线
  三期在途测试，非本线辖区、与本线改动无依赖关系，登记
  备协调线对账。

## 5. lib.rs 改动清单（逐条）

- 新增 `pub mod prescan_bindings;`、`pub mod prescan_consts;`、
  `pub mod symbols;`（按字母序插入模块表）。
- 顶部文档新增 Phase 3 (SAB-C) 段落（模块与行段对照、接线
  说明）。其余声明未动；Cargo.toml 依赖边**未动**。

## 6. 新分歧 / 反馈单

- 分歧新增 3 条（sab.md）：SAB-D23（b，borrowed 每函数扫描
  + 导入宏 plan 缺口）、SAB-D24（b，trait 事实来源/CheckInfo
  面）、SAB-D25（c，MIN/-1 折叠击穿）；已核查排除新增 3 项
  （hex 反斜杠、shl 位模式、arr 段级对拍）。sab.md 总账：
  (a) 2 / (b) 10 / (c) 13 = **25 条**，已核查排除 7 项。
- 反馈单 1 张：**FB-025**（号段 025–028 内）——顶层常量
  折叠对 i64::MIN / -1 无防护，真插件文本/SAB 双路
  **rc=136 SIGFPE 核心转储**实证（divmin 探针）；机制横跨
  sab_codegen foldTopLevelBinaryConst 与 lowering_rules
  intConstantExprValue 两处同形代码；建议按既有溢出口径
  `catch return null` 补防护。端口以 checked_div/rem 放弃
  折叠，不复刻陷阱。

## 7. D 组评估（未收到 INT-2 解锁通知，只评估不动工）

对象 D1 `surface_load.rs`（Zig 1273–1589），已通读原文：

- **构成**：`readStdSurfaceFile`（1273–1352，四级回退链取
  `std_surface.sla_meta`：SLA_STD_DIR → ~/projects 硬路径 →
  self-exe 相对三候选 → cwd 四候选含 `/home/vscode/...`
  硬编码）、`resolveSaStdRoot`（1359–1413，烘焙
  `sa_std_source_dir` 优先——与主线「SAB 优先插件烘焙目录」
  实证一致；其后 SA_STD_DIR → SCI_ROOT → home 两根 → cwd
  六候选；有效性 = 四个必需文件可打开；全败 FileNotFound
  响亮）、`loadStdSurfaceRules`（1483 起，58 行 .sla_meta
  的逐行规则解析；文件缺失时 `orelse return` **静默以零
  规则继续**——SAB-D02 家族新实例，D1 落地时须按 D02 既定
  方针响亮记录候选失败与最终落点）。
- **接线前置**：① 烘焙目录在 Rust 侧无对应物——需 sla-plugin
  构建期注入（build.rs/env），属协调线接线面，本线不动；
  ② D1 单独落地不可观察（消费者 D4 查询/D5 预载/gen_call
  表面路径仍全响亮），应与 D4/D5 同批开线；③ 规则文件
  fixture 现成（sla_std/std_surface.sla_meta，58 行），解析
  单测可先行以真文件 + 真产物指纹（哪根胜出须写入构建
  指纹）为对照；④ INT-2 的合同段（.sai→extern 注册）与 D 组
  表面是两套机器（协调线 D-A2 裁决），互不阻塞，但
  CheckInfo 若因 INT-2 扩面（如 trait 事实，见 SAB-D24），
  D1 的根解析结果宜经同一接线面注入 driver。
- **结论**：解锁通知到达后，D1 可按上表直接开工，无未决
  设计问题；预计文件 surface_load.rs ≤700 行可容纳
  （1273–1589 共 317 行 Zig，含解析器全家）。
