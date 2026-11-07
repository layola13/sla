# SAB 拆分图 — sab_codegen.zig (20,024 行) → sla-codegen-sab

施工图版本：2026-11-07（子系统线 E 一期）。对照 Zig 源：
`sa_plugin_sla/src/sab_codegen.zig`（本地参照树 20,024 行 / 733 方法 /
`Codegen` 结构 294–19186 + 文件尾 `generate` 包装与内联测试 19187–20024）。
编码器本体不在该文件内，位于 sci `src/sab.zig`（写路径）与
`src/common/{instruction,signature,const_decl,atomic,upstream_loc}.zig`；
**格式口径以远端现头 sci `e82d2865` 为准**（参照树曾滞后，见
divergences/sab.md SAB-D01）。

## 读图说明

- 每模块给出：Rust 文件名、对应 Zig 行段、职责、依赖方向。
  行段为方法 span 汇总（含函数体），Rust 实现行数会不同
  （适配层通常更短；Zig 的 `anyerror`+HashSet 记账转成类型化枚举后
  局部更长），但**每文件 ≤700 行**为硬约束，超限先拆后写。
- 依赖方向：`ir`/`leb128`/`pool`/`encode` 为底层（不依赖任何 gen 模块）；
  `emit` 主干依赖 `ir`；全部 `gen_*`/`surface_*` 模块依赖
  `state`+`release_*`+`emit`，互不反向依赖；`driver` 只做编排。
- 状态标记：✅ 一期已实现（切片内）；◐ 部分实现；○ 二期起按图开线。

## A. 基础层（sci 格式与 IR）

| # | 模块 | 对应源 | 职责 | 状态 |
|---|------|--------|------|------|
| A1 | `leb128.rs` | sci sab.zig 104–129 写侧 | ULEB128/SLEB128 编解码 | ✅ |
| A2 | `ir.rs` | sci common/{instruction,signature,const_decl,atomic}.zig | Operand/Instruction/FunctionSig/ConstDecl 类型与枚举序号（线格式即声明序） | ✅ |
| A3 | `pool.rs` | sci sab.zig 921–986, 131–144 | 字符串池：插入序去重 + 四段收集顺序（符号→指令文本→签名→常量） | ✅ |
| A4 | `encode.rs` | sci sab.zig 写路径全量 | Module→SAB v4 字节；段序 1 池/2 签名/3 常量/4 指令 | ✅ |
| A5 | `error.rs` | sab_codegen.zig 58–65 | 错误集镜像（Unsupported/ConflictingConstDecl/MissingType/InvalidStringLiteral）+ 编码器 InvalidModule | ✅ |

## B. 代码生成状态与主干

| # | 模块 | Zig 行段 | 职责 | 状态 |
|---|------|----------|------|------|
| B1 | `profile.rs` | 17–57 | SLA_SAB_PROFILE 环境门控的分段计时打印 | ○ |
| B2 | `types.rs` | 66–293 | 辅助结构：Local/借用守卫值/分支快照槽/宏捕获/StdSurfaceRule/StdImportModule/DecodedModuleLocalRemap 等 | ✅ 二期落地（369 行，结构全集） |
| B3 | `state.rs` | 294–534 | `Codegen` 字段全集 + init/deinit（Rust 侧拆为若干所有权明确的子状态，见 SAB-D09） | ✅ 二期落地（255 行，ProgramState/FrameState/CodegenState + RegState 枚举收敛） |
| B4 | `driver.rs` | 535–632 | `generate` 编排：预扫→表面预载→future 助手→trait vtable→逐 decl 分发→逃逸闭包→契约 extern→encodeProgramWithConsts | ◐ 二期本段范围 ✅（driver.rs 642 + driver_gen.rs 532；fixture01 级编排路径已通、generate_from_program 真实驱动；C/D/F–I 组调用点为响亮未接线，见 SAB-D19） |
| B5 | `emit.rs` | 1103–1139, 3615–3622, 3848–4030 节选, 19172–19186 | 发射主干：intern/makeInst/appendInst + 原语发射器（assign/op/ptr_add/alloc/load/store/take/jmp/label/branch/return/panic） | ◐ 主干已实现（intern/命名/decl/assign/op/call/label/jmp/branch/return/panic/字符串常量）；二期补 emit_assign_reg/emit_panic_reg/sig 元信息 setter 与 set_sig_id 双轨钩子（SAB-D17） |
| B6 | `literals.rs` | 1140–1272 | 字符串字面量解码/转义（`\xHH` 大写）+ utf8 常量追加（尾 NUL） | ◐ 转义/解码已实现；常量标签管理在 emit.rs |

## C. 预扫与符号

| # | 模块 | Zig 行段 | 职责 | 状态 |
|---|------|----------|------|------|
| C1 | `prescan_consts.rs` | 633–854 | 顶层全局标量/数组常量收集与折叠（foldTopLevelBinaryConst）+ vtable 预扫 | ○ |
| C2 | `prescan_bindings.rs` | 855–934, 2954–3056 | 已赋值/多重 let/借用绑定收集；recordReg/finishFunctionRegs | ○ |
| C3 | `symbols.rs` | 935–1102 | internStable、绑定寄存器号、函数名 mangle、trait vtable 条目发射 | ○ |

## D. std 表面（surface）基础设施

| # | 模块 | Zig 行段 | 职责 | 状态 |
|---|------|----------|------|------|
| D1 | `surface_load.rs` | 1273–1589 | std surface 文件定位与规则解析（多根回退链——静默吞错富矿，见 SAB-D02） | ○ |
| D2 | `layout.rs` | 1590–1706, 1776–1782 | primType/storagePrimType/尺寸与字段布局适配（重活在 lowering_rules；本层为适配+缓存，见 SAB-D10） | ○ |
| D3 | `copy_facts.rs` | 1783–1883 | typeHasCopyDerive/typeIsCopy/shallowCopy 等拷贝事实（含 catch{} 缓存，见 SAB-D05） | ○ |
| D4 | `surface_query.rs` | 1884–1997 | 规则查询（findStdSurfaceRule 等）+ 依赖预载入口 | ○ |
| D5 | `surface_preload.rs` | 1998–2317 | 按块/节点遍历预载表面依赖；future/thread 使用扫描；stackAllocSize | ○ |

## E. 局部量、分支状态与释放引擎

| # | 模块 | Zig 行段 | 职责 | 状态 |
|---|------|----------|------|------|
| E1 | `opkinds.rs` | 2318–2390 | cast/二元运算的 scalarBinaryOpKind/opKindForBinary 映射 | ○ |
| E2 | `locals.rs` | 2391–2511 | pushLocal 家族 + beginFunction/finishFunction 帧管理 | ○ |
| E3 | `branch_state.rs` | 2430–2767 | 分支发射器状态快照 clone/merge/restore；RefCell 分支句柄合并槽；借用临时快照 | ○ |
| E4 | `release_meta.rs` | 2768–2953 | 释放元数据：单寄存器释放集、借用地址临时、Result 槽 RefCell 伴随、非拥有标记 | ○ |
| E5 | `release_locals.rs` | 3057–3377 | 参数清理动作、分支平衡释放（emitBalanceReleaseLocal，见 SAB-D04）、栈局部释放、循环跳转清理、作用域块释放 | ○ |
| E6 | `captures.rs` | 3378–3577 | 闭包逃逸捕获收集（collectEscapedCaptures*）+ localReg/localType 查询 | ○ |
| E7 | `release_emit.rs` | 3578–3847 | 表达式结果释放、RefCell 借用释放、分支 release/move 发射、future 状态 vtable 转移、readiness（见 SAB-D11） | ○ |

## F. 解码模块导入（std 宏片段机
...[truncated 5832 chars]