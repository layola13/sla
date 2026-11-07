# 分歧登记 — SAB 线（sla-codegen-sab）

登记纪律：差分重构中发现的分歧**逐条登记、不静默迁就**。分类：
(a) 输出分歧——同一输入两实现产出不同（字节/结构）；
(b) Rust 纪律强制的结构性重构（语义等价、形态不同）；
(c) Zig 原文可疑构造（静默兜底/吞错/别名与所有权记账嫌疑）——
此类只登记 + 在端口中**原样复现**（若影响字节），修复归主线裁决，
本线不擅自「修好」以免制造 (a) 类分歧。

行号均指 `sa_plugin_sla/src/sab_codegen.zig`（本地参照树，
20,024 行）；格式类条目另注 sci 现头 `e82d2865` 出处。

## (a) 输出分歧

### SAB-D01 参照树版本漂移导致签名记录差 1 字节（已定位已修正）
- 现象：首轮对拍 minimal 模块 Rust 75 字节 vs Zig（场地 sci）76 字节，
 首差在签名段长度字节；fixture01 模型差 2 字节（两条签名各 +1）。
- 根因：本地参照树 sci 停在 `1ec669ad`，现头为 `e82d2865`；现头
 `FunctionSig` 多序列化一个字段 `is_variadic`（sab.zig writeFunctionSigs
 在 `should_panic` 后多一行 `writeByte(if (item.is_variadic) 1 else 0)`），
 参照树无此字段。同批漂移还有枚举尾部追加：`InstKind` 增
 `va_start/va_arg/va_end/gpu_decl`（=36..39）、`FunctionKind` 增
 `gpu`（=5）——原文注释明言「SAB 按原始字节编码、既有值不得移动」
 才追加在尾。
- 处置：端口已按现头补齐（ir.rs/encode.rs），黄金向量 75→76 字节；
 复测双模块与 Zig 探针 **sha256 全同**（minimal `000fb0e6…`、
 fixture01 模型 `31fc5e86…`）。教训与主线 AGENTS.md「取材先对版」
 同族：差分基准必须是远端现头，本线参照树已标注过期。
- 附带观察（非线格式，登记备查）：现头 sab.zig 的 raw_text 合成
 为 call_indirect 追加 carried fn-ptr 签名后缀（C2
 sab_carried_indirect_sig）；signature.zig 新增 parseParamList
 （`...` 变参解析）——属文本线/ lowering 线辖区。

## (b) 结构性重构

### SAB-D06 `catch unreachable` 与裸 `@intCast` 面
- 原文：`catch unreachable` 3 处（6497 allocator.create、
 8901 closure_param_regs.put、9523 tc.expr_types.put）；全文件
 `unreachable` ×16、`@intCast` ×80。
- 处置：Rust 端一律 Result 传播 + 显式范围检查（溢出报
 InvalidModule），不以 panic 代替错误通道；序号转换集中在 ir.rs。

### SAB-D08 寄存器身份 = 名字键 + 函数全局，靠补丁集打补丁
- 原文：寄存器以名字 intern 为全局 id（字段注释 ~306–309 解释
 multi_let_bindings 的存在理由、~356–369 解释 stack_alloc_emitted
 等补丁集），另有 inferred_string_slice_locals、
 explicit_ptr_string_locals 等推断回填集。
- 处置：Rust 端寄存器身份与「第几代绑定」分离建模（binding 代次
 入类型），补丁集不平移；属 E3/E4/E5 模块二期设计约束。

### SAB-D09 所有权状态 = 巨型结构上的散列标志集合
- 原文：non_owning_regs / borrow_result_regs / released_regs /
 stack_alloc_regs / borrow_address_temps / refcell_borrow_values /
 mutex_guard_values / rwlock_guard_values /
 result_slot_refcell_* 等十余个 HashSet/Map 并存在同一 `Codegen`
 上，同一寄存器可同时属于多个集合，合法组合靠约定维持。
- 处置：Rust 端以每寄存器状态枚举建模（Owned/Borrowed/View/
 Guard/RefCellBorrow/…互斥），非法组合不可表达；映射表在
 decomposition 的 state.rs（B3）条目下二期给出。

### SAB-D15 本端口错误携带原因字符串（仅诊断面）
- 原文：`UnsupportedSabDirectFeature` 无 payload，原因只在环境
 变量开启时走 stderr（见 SAB-D07）。
- 处置：本端口 `Error::UnsupportedSabDirectFeature(String)`
 无条件携带原因。只影响报错文本，不影响任何输出字节；登记
 为有意分歧，避免后人「对齐」回无声形态。

### SAB-D16 单结构 733 方法 → 50 模块
- 原文：`Codegen` 单 struct（294–19186）承载全部职责 + 文件头
 辅助结构与文件尾内联测试。
- 处置：即 docs/sab-decomposition.md 的模块图本身；方法按职责
 拆分、依赖单向。形态分歧、语义目标等价。

## (c) Zig 原文可疑构造

### SAB-D02 std 表面文件定位：多根回退链整段吞错
- 原文：`readStdSurfaceFile`/`resolveSaStdRoot` 中 1282、1316、
 1363、1370 行 `else |_| {}`——环境变量/文件系统查询失败被整体
 吞掉，沿 SLA_STD_DIR → `~/projects` 路径 → cwd 相对候选静默
 回退。与主线实证「SAB 静默测到旧 std、绿证作废」同族：定位
 失败与「用了另一份 std」在现象上不可区分。
- 端口方针：surface_load.rs（D1）实现时每根候选的失败原因必须
 响亮记录，最终落点写入构建指纹；不复刻静默链。

### SAB-D03 结构体字面量/更新路径 defer 中吞释放错误
- 原文：9382 与 17844 行 `releaseExprResultIfNeeded(update_expr.?, reg) catch {};`
 位于 `defer` 内——所有权释放失败（泄漏/双重释放族）被完全吞掉。
- 端口方针：release_emit.rs 复现调用结构但错误必须上抛/登记，
 不许 `catch {}` 等价物。

### SAB-D04 emitBalanceReleaseLocal 跳过平衡释放
- 原文：~3159 行（函数体 3139–3176）`primType(ty) catch return;`——
 取不到类型即静默跳过参数平衡释放，而相邻注释明言此释放是为
 避免 PhiStateConflict 而设。类型缺失时恰是最需要响亮报错处。
- 端口方针：MissingType 错误上抛（error.rs 已备此变体）。

### SAB-D05 拷贝派生缓存吞错
- 原文：typeHasCopyDerive 的缓存路径 1791/1795/1805 行 `catch {}`；
 另 16446 行 closure_param_regs 恢复处 `catch {}`。
- 性质：较 D02–D04 为轻（缓存未命中方向），但「推导失败」与
 「无拷贝派生」被合并为同一结果，登记待二期细分。

### SAB-D07 不支持原因默认不可见
- 原文：`traceSabUnsupported`（6794–6799）仅当环境变量
 `SLA_SAB_TRACE_UNSUPPORTED` 存在时才把原因打到 stderr，且其
 环境读取自身 `catch return false`（6795）；`traceUnsupported`
 （6800）共 39 个调用点。默认形态下失败只有无 payload 的
 `UnsupportedSabDirectFeature`——失败是响亮的，原因默认是哑的。
- 端口方针：见 SAB-D15。

### SAB-D10 布局查询把「类型不可映射」与「无此字段」合并为 null
- 原文（嫌疑）：tupleFieldLayout（1621）/arrayElementLayout
 （1634）/fieldLayoutFromDecl（13366）把 storagePrimType 失败
 映射为 `null` 返回，与「字段不存在」共用同一通道。
- 理由：布局是偏移计算的源头，null 的两种含义在下游不可分辨；
 与 SAB-D14 的历史案型同构。二期 layout.rs 须以枚举区分。

### SAB-D11 future readiness 的 unknown 兜底
- 原文（嫌疑）：futureReadinessForState（3815）
 `orelse .unknown`——状态缺失时默认「未知」继续发射，而非报错。
- 理由：readiness 决定 future 对象物化形态，默认值若被下游当
 确定事实消费即成静默降级。二期 async 模块须复核消费点。

### SAB-D12 检查器无类型时按字面量形状伪造类型
- 原文：exprTypeOrFallback/literalFallbackType（6805–6822）——
 检查器未记录表达式类型时，codegen 按字面量外形编造一个
 primitive 类型继续发射。
- 理由：类型事实的源头是检查器；codegen 侧伪造使「检查器漏
 类型」从响亮失败变成潜在错码。二期 stmt_core 须改为
 MissingType 上抛或显式登记每次回退。

### SAB-D13 emitBranch 把 then 标签重复写进 operands[2]
- 原文：emitBranch（12537–12545）operands[1] 与 operands[2]
 同为 `.label = intern(then_label)`，operands[3] 为 else 标签。
- 核查：sci 全部消费者（emit_llvm_llvmc、emit_js、interp）只读
 operands[1]（真）与 operands[3]（假），operands[2] 无人读——
 今日为死数据；但解码合成的文本镜像呈 `br a, a, b` 三目标
 形态，任何未来按 [2] 语义（如 phi/merge 目标）实现的消费者
 会静默继承 then 标签。
- 端口方针：**字节级原样复现**（emit.rs::emit_branch，测试
 固定此形态），等主线裁决是否修 Zig 本体；本线不擅改。

### SAB-D14 布局吞错家族（历史案型 + 当前实例面）
- 历史实证（主线 AGENTS.md 2026-11-02）：sab_codegen 对无声明
 Vec 调 fieldLayout 取 len 偏移必报错，被 `else |_| <字面量>`
 吞成字面量 8（那是 cap 的偏移），致 SAB 深 drop 按 cap 迭代
 未初始化槽 SIGSEGV。
- 当前树核查：Vec 相关站点（12952、17686、17707）已正确改用
 `lowering_rules.VecAbi.len_offset`/`ptr_offset` 常量，该案型
 在这些站点似已修复；家族的活实例为 SAB-D02/D04（吞错点）
 与 SAB-D10（null 合并）。登记为模式类，二期逐模块普查
 `else |_|`、`catch {}`、`orelse <字面量>` 三种形态。

## 已核查排除（checked & cleared，不计入分歧数）

- foldTopLevelBinaryConst 溢出返回 null（796–798）：只是放弃
 常量折叠、回退正常发射，方向安全。
- profile 打印处 `catch {}`（40/41/54/55）：仅调试输出，失败
 无语义后果。
- sci 编码器池查找严格报错：sab.zig `poolId`/writeOperand 的
 text 查不到池即 `error.InvalidTag`，无 orelse 0 式静默回退
 （本端口 InvalidModule 与之同口径）。
- emitAssignReg 中 markConsumed 两段叠加（3884 附近）：集合
 插入幂等，冗余但无害；其注释揭示的「assign 消费源寄存器 +
 七类生命周期豁免」规则已并入 SAB-D09 的状态建模输入。

## 计数

- (a) 输出分歧：**1**（SAB-D01，已修正并复测全同）
- (b) 结构性重构：**5**（D06、D08、D09、D15、D16）
- (c) 可疑构造：**10**（D02、D03、D04、D05、D07、D10、D11、
 D12、D13、D14；其中 D10/D11 标「嫌疑」待二期复核定性）
- 合计登记 **16** 条；另已核查排除 4 项。
