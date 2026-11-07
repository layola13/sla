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

## 二期 B 组新增登记（2026-11-07，SAB 线二期）

### SAB-D17 sig.id 双轨：genFuncSig 用序号、decl 用 intern id（a 类；已双轨镜像）
- 规则（逐条，行号为参照树 `sab_codegen.zig` 现行编号）：
 ① `genFuncSig`（5963）在入口先算 id：5964 行
 `id = function_sigs.items.len + test_sigs.items.len`（两表
 **当时**长度之和，即此前经此路径建出的 sig 总数），5985 行
 写入 `FunctionSig.id`；此 id 还决定 test 的 llvm_name
 （5995 行 `"_saasm_test_{d}"` 以同一 id 格式化）。
 ② sig 本体随后才入表：普通/异步/test 路径在 genFuncSig 返回
 后以 `sig_idx = function_sigs.items.len` 捕获下标再 append
 （如 genFuncDeclNamed 6214-6216、genTestDecl 6478-6480）；
 extern 路径 genExternDecl（6267-6272）先 appendDeclInst 后
 append——三路 id 均为「入表前计数」，故第 n 个 sig 的 id 是
 0 起的建出序号（受 ③ 的跳号影响）。
 ③ test 双计数：genTestDecl 在 6480 行把 fsig 追加进
 function_sigs，又在 6493 行把同一 sig 追加进 test_sigs——
 一个 test 使计数器前进 2，紧随其后的 sig.id 跳过一个数。
 ④ decl 操作数是另一轨：`declInstForSig`（6007）6008 行
 `id = intern(fsig.name)`（符号表 intern 序号），6017/6018 行
 operands[0]=Symbol(id)、operands[1]=Func(id)，与 fsig.id
 无关；多函数时两轨系统性不同，单函数时恰同为 0。
 ⑤ 同公式的旁路：appendGeneratedFuncSig（5601-5612）生成型
 sig 亦用 function+test 两表长度和。不同公式的边界路径
 （driver 子集外，仅登记不镜像）：makeContractExternSig
 （6300）id 先置 0（6318），emitContractExternDecl（6335）
 以 6339 行 `function_sigs.items.len`（仅主表）覆写、
 emitReferencedContractExternDecls 以排序下标覆写（6404），
 且该路径末尾 renumberFunctionSigs（6434-6445）会把全部
 sig.id 重写为主表下标——即 Zig 本体在 contract extern
 介入时亲手抹平双轨；cloneModuleFunctionSig（4866-4868）/
 cloneDecodedModuleFunctionSig（4887-4903）的克隆 sig 亦只
 用主表长度。以上边界均在 driver 子集之外（contract extern
 调用与模块克隆在 driver 内响亮拒绝），不影响本条镜像面。
- 真产物裁决（2026-11-07，场地三真 Zig 插件 `sa sla sab
 build`，证据级别=**真产物字节级**）：程序 A 三函数
 add/helper/main → multi.sab 662 B、sha256 `1a27f1d1…`：
 sig.id=0/1/2（建出序号），decl 操作数=0/5/9（intern id，
 池序 sla__add=0、sla__helper=5、main=9），双轨实锤。程序 B
 函数+@test+函数 → multi2.sab 573 B、sha256 `6df45c9f…`：
 sig.id=0/1/**3**（test 双计数跳号在真产物中证实），decl
 操作数=0/5/7，test 的 llvm_name `_saasm_test_1` 亦以序号
 id 构成。本端口 driver 产出与真产物在 sig.id 字段逐项
 一致（整模块字节不等是 SAB-D19 的既登记省略面，与本条
 无关）。
- 处置（协调线裁决：按 Zig 双轨镜像以对齐真产物字节，不
 统一）：driver 的 `begin_sig` 按 ①-③ 给 sig.id 编号
 （function_sigs 下标 + 进入时 test_sigs 长度，覆写在
 append_decl_inst **之后**，使 decl 操作数保持 intern id），
 经主干新增的加法钩子 `ModuleBuilder::set_sig_id` 落地；
 B5 主干默认口径（一期模型/generate() 路径的 intern id）
 与既有 API 签名不变，一期探针黄金向量不受影响。回归：
 driver_e2e 新增 P3（func→test→func）逐字节对拍手工双轨
 模型（sig.id=0/1/3）。Zig 侧双轨气味仍挂 FB-015，本条在
 本端口分类为 (a) 类**已镜像**（怪癖照录，no improvements）。

### SAB-D18 逃逸闭包条目以 AutoHashMap 迭代序发射（c 类）
- 原文：`escaped_closure_entries`（~330）为
 `AutoHashMap(*const ast.Node, …)`，`emitEscapedClosureEntries`
 （~5900）以 `valueIterator()` 迭代序逐条发射 wrapper/worker——
 发射序由指针哈希决定、非源码序，字节可随分配环境漂移。
- 端口方针：state.rs 以插入序 `Vec<(node_key, entry)>` 保存，
 发射序由插入序决定（哈希序纪律）；已落单 FB-016。

### SAB-D19 driver 子集的发射省略面（b 类，范围登记非缺陷）
- 二期 driver（B4 子集）相对 Zig 全量 gen 的有意省略，全部属
 E 组未接线范围，逐项在此登记以免被误读为「已对齐」：
 ① 释放/清理发射全缺（`releaseOpenLocals`/`emitRelease`/
 分支平衡释放/循环跳转清理）——纯标量 Copy 子集内无释放指令，
 与 phase-1 手工模型口径一致；② 函数入口不发 `L_ENTRY` 标签
 （模型口径；测试 decl 的 `L_TEST_ENTRY` 例外，Zig genTestDecl
 确有此标签、driver 已发）；③ `bindingNeedsScalarReassignSlot`
 的栈槽路径未用，标量 let/assign 走普通寄存器 assign
 （genLetFromValue 的兜底臂形态）；④ if 值形态（genIfValue 的
 alloc/store/load 结果槽）响亮拒绝而非静默误降；⑤ while 的
 分支释放/状态恢复（restoreReleased 等）未接，循环体局部靠
 locals 截断收口。以上各项在触及非标量/值形态时均响亮报错。

### SAB-D20 B2 辅助结构的所有权形态（b 类）
- `StdImportModule`：Zig 存 `FlattenResult` + 独立 sig 列表，
 本端口直接存四段模块内容，`module()` 重组出同一 `sab.Module`
 视图（字节等价、形态不同）。
- `MacroLocalChange.old`：Zig 在 `had_old=false` 时为未初始化
 内存，本端口为 `Option<MacroLocalBinding>`（None 即无旧值）；
 `MacroArgBinding.ctx` 的裸回指针改为 `Rc<RefCell<…>>` 共享
 句柄。均为所有权纪律强制的形态适配，无输出影响。

### SAB-D21 测试 decl 命名：driver 从 Zig、phase-1 模型为占位（b 类）
- phase-1 `fixture01.rs` 手工模型自承其测试函数名
 `test_hello_world` 是占位猜测、无 `L_TEST_ENTRY` 标签。
 Zig `genFuncSig` 的 test 臂以 `"{name}"`（带引号原文）为符号名、
 `genTestDecl` 发 `L_TEST_ENTRY` 标签。driver 以 Zig 为准
 （引号名 + 标签 + `_saasm_test_{id}` llvm_name 口径由 sig 字段
 承载）；模型文件保留作一期编码器对拍证据，不再作 AST 路
 命名口径。两者差异属模型占位被真实口径取代，非 Zig 缺陷。

### SAB-D22 panic 非单参数形态静默写死代码 "1"（c 类）
- 原文：`genCall` panic 臂（15167–15180）的 `else` 兜底把
 操作数写为文本 `"1"`，不区分无参/多参；多参时实参不求值、
 代码被替换，无诊断。静默兜底家族（与 SAB-D14 同科）。
- 端口方针：driver 字节级镜像（同为代码 1）+ 落单 FB-017。

## 三期 C 组新增登记（2026-11-07，SAB-C 线）

### SAB-D23 borrowed_bindings 是每函数扫描、且依赖未移植的导入宏计划（b 类）
- 原文：`collectBorrowedBindingsInBlock/InNode`（2954–3043）
 不在 `generate` 预扫内，而是每个函数/测试入口路径在
 `prepareMultiLetBindings` 之后调用（6060、6117、6152、6212、
 6477、7690、7909、7950），集合由 `beginFunction` 先行清空。
 本端口初版登记曾据截断的 grep 输出误判其为死代码——已更正，
 谨记：全文件检索不许带 head 截断下结论。
- 移植形态：C2 walker 逐字移植（prescan_bindings.rs），driver
 在 gen_func_decl / gen_test_decl 同一点接线。walker 内的
 `planImportedMacroCall`（lowering_rules 3109–3123）sla-lowering
 尚未移植，端口以 `ImportedMacroPlans` trait + 拒绝一切的
 默认 provider 占位：driver 子集内导入宏调用在 gen_call 处
 本就响亮拒绝，但「仅在借用扫描中出现导入宏地址实参」的
 程序会少收集——待 lowering 补 plan 后销此项。

### SAB-D24 C3 的 trait 事实来源与 CheckInfo 面（b 类）
- 原文：`appendTraitVTableEntries` 读 `tc.traits`（含导入展开
 填充的 trait）与 `tc.funcs` 的 decl-only 标记。
- 移植形态：CheckInfo 未暴露 trait 表，driver 侧的 TraitFacts
 由程序自身声明组装（TraitDecl 节点 + impl 方法的 decl-only
 形态按 mangle 名登记），另以 `check.func(mangled)` 复核。
 程序本地 trait 与 Zig 逐项一致（场地真产物 vt 探针对照：
 字面量 `vtable { name = @sla__Sprite__Named_name }` 与
 `VT_Sprite_Named` 逐字同）；**导入 trait** 在端口侧会响亮报
 「unknown trait」（与 Zig 的 tc.traits 未命中同为
 UnsupportedSabDirectFeature，但触发面更宽）——待 INT-2 合同
 段把 trait 事实接进 CheckInfo 后收窄。

### SAB-D25 顶层常量折叠 MIN/-1 除法击穿编译器（c 类，已落 FB-025）
- 原文：`foldTopLevelBinaryConst`（785–835）的 div/mod 臂仅防
 除零（`if (ri == 0) return null`），不防 i64::MIN / -1 溢出，
 与同函数 add/sub/mul 的 `catch return null` 口径不一；
 lowering_rules 的 `intConstantExprValue` 同形。
- 实证（2026-11-07 场地三现头插件）：四行 const 链把 MIN 与
 -1 折到一起后，`sa sla sab build` 与 `sa sla build` **双路
 rc=136 SIGFPE 核心转储**、无诊断。详见 FB-025。
- 端口方针：checked_div/checked_rem 放弃折叠（与既有溢出
 口径一致），不复刻陷阱。

## 三期 D 组新增登记（2026-11-07，SAB-D 线）

### SAB-D26 dep 确保的排队分支被无条件采用（b 类）
- 原文：`ensureStdDeps`（6108–6136）按 `in_function_body`
  分流——函数体内排队进 `pending_std_deps`（由
  `flushPendingStdDeps` 在函数收尾时 append），函数体外
  （正是 `generate` 的 preload 时点）走 `appendStdDepsNow`
  立即经 F 组解码模块机器 append。
- 移植形态：F 组未落地，本端口的 ensurer 无条件走排队分支
  （`surface_query::PendingStdDeps`），preload 发现的全部
  deps 以请求序入队、可观测、不丢弃；与 Zig 的差异仅在
  append 的**时点**（延后到 F 组接上 `flushPendingStdDeps`
  消费端为止）。下游 gen_call 的表面展开本就响亮拒绝，
  故当前不可观察于任何输出字节。

### SAB-D27 浅拷贝枚举臂因 CheckInfo 无枚举表不可达（b 类）
- 原文：`typeIsShallowCopyCallArgValue` 的 user_defined 臂
  有 `tc.enums.contains(name) → true`（纯枚举 POD 形）。
- 移植形态：Rust `CheckInfo` 表面未暴露枚举表，D3
  （`copy_facts.rs`）的该臂不可达，枚举型嵌套调用实参落到
  struct-decl 路径并答 false——触发面比 Zig **窄**（仅
  「用户枚举型作为嵌套 call-arg 值」一形），从不更宽。
  待 INT-2/CHK 线把枚举事实接进 CheckInfo 后补齐销账。

### SAB-D28 表面规则文件缺失静默零规则（c 类，SAB-D02 家族新实例，已落 FB-026）
- 原文：`loadStdSurfaceRules`（1551–1554）
  `const source = (try self.readStdSurfaceFile()) orelse return;`
  ——四级回退链全空时静默以零规则继续，其后一切表面查询
  查无规则、统一以无 payload 的 Unsupported 失败，与「表面
  本就不支持」不可区分。同文件 `resolveSaStdRoot` 全败时
  却 `return error.FileNotFound` 响亮失败，两条定位链口径
  不一。
- 端口方针：控制流与零规则结果原样镜像；回退链每根候选
  的成败与最终落点写入 `SurfaceLoadLog` 随程序状态保留
  （D02 既定方针的响亮记录），不改任何发射字节。

## 已核查排除（checked & cleared，不计入分歧数）

- foldTopLevelBinaryConst 溢出返回 null（796–798）：只是放弃
 常量折叠、回退正常发射，方向安全。
- hex 字面量的反斜杠数（2026-11-07 三期实证）：emitGlobalArrayConst
 的格式串在源码中写作 `"\\\\x{x:0>2}"`（双反斜杠），但 sci 的
 `sio.managedPrint` 将其折叠——真产物池文本为**单**反斜杠
 （`hex:\x01...`，字节列实证 [104,101,120,58,92,120,48,49]）。
 本端口初版按 std.fmt 直读写成双反斜杠（本移植缺陷），经
 场地真产物字节对照抓出并修正；Zig 侧行为本身与 SA 文本
 形态一致，不作分歧。
- 移位折叠的溢出位模式（同轮实证）：`3 << 62` 真产物折叠为
 -4611686018427387904（位模式截断），与端口 wrapping_shl 一致。
- arr 探针的段级对拍（同轮实证）：`const TABLE: [i32; 3] =
 [1, 2, 16909060]; fn main() -> i32 { return 1; }` 真产物
 225 B（sha256 a114ec03…）与端口输出 197 B（sha256
 01862628…）逐段比对——签名段同长（19 B）、常量段同长
 （21 B，C1 数组常量 decl 逐字节同），差异仅 `L_ENTRY`
 （池 +8 B、指令段 +20 B），即 SAB-D19② 的既登记省略面，
 非 C 组新分歧。
- profile 打印处 `catch {}`（40/41/54/55）：仅调试输出，失败
 无语义后果。
- sci 编码器池查找严格报错：sab.zig `poolId`/writeOperand 的
 text 查不到池即 `error.InvalidTag`，无 orelse 0 式静默回退
 （本端口 InvalidModule 与之同口径）。
- emitAssignReg 中 markConsumed 两段叠加（3884 附近）：集合
 插入幂等，冗余但无害；其注释揭示的「assign 消费源寄存器 +
 七类生命周期豁免」规则已并入 SAB-D09 的状态建模输入。

## 计数

- (a) 输出分歧：**2**（SAB-D01，已修正并复测全同；SAB-D17，
 sig.id 双轨已按真产物逐字镜像、driver 输出与 Zig 在该字段
 一致，Zig 侧气味仍挂 FB-015）
- (b) 结构性重构：**12**（D06、D08、D09、D15、D16、D19、
 D20、D21、D23、D24、D26、D27）
- (c) 可疑构造：**14**（D02、D03、D04、D05、D07、D10、D11、
 D12、D13、D14、D18、D22、D25、D28；其中 D10/D11 标「嫌疑」待复核定性）
- 合计登记 **28** 条；另已核查排除 7 项。
- D 组核查附记（2026-11-07，SAB-D 线）：`resolveSaStdRoot`
  的烘焙目录优先序与主线「SAB 优先插件烘焙目录」实证一致，
  非分歧；现头 `std_surface.sla_meta` 全量 57 条规则
  （associated 16 / method 23 / function 5 / constructor 4 /
  fallible_method 1 / index 3 / index_address 3 /
  index_assign 2）均在 D1 解析器覆盖内；场地真插件对照：
  println 探针产物 1002 B（sha256 `f7ef7511…`）、Vec::new
  探针产物 798 B（sha256 `131c90aa…`）——两程序在本端口均
  死于 gen_call 表面展开（D 组）/ 其后 F 组片段机器，
  死点已在 driver_e2e_d 中钉死。

## 四期 F 组新增登记（2026-11-07，SAB-F 线）

### SAB-D26 销账注（2026-11-07，SAB-F 线）
- F 组落地后，本条所述「无条件走排队分支」已解除：driver
  step 2 的 preload 改经 `MachineStdDeps`（F 机器的
  `ensureStdDeps` 镜像，函数体内排队 / 函数体外立即
  `appendStdDepsNow`），函数尾 `flushPendingStdDeps` 已接线
  （gen_func_decl / gen_test_decl）。`PendingStdDeps` 保留为
  排队分支的单元形态与 D4 测试载体。append 的时点差异自此
  消除；新的不可观察性只在 flattener 边界（SAB-D31）。

### SAB-D29 解码器不建模 raw_text（b 类）
- 原文：sci `sab.zig` 的 `Module` 指令/常量带 `raw_text`：
  v2/v3 流内嵌、v<4 由 `synthesizeRawText` 解码时合成、
  常量由 `readConstDecls` 合成 `@const {name} = {literal}`。
- 移植形态：本仓 IR 自一期起即不建模 `raw_text`（写路径从
  不序列化它）；F1/F2 解码器照常消费 v2/v3 的 raw-text 槽
  位但不保存，常量合成文本不保存，`synthesizeRawText` 整段
  跳过。后果仅在诊断/反汇编面：畸形旧版模块（major<4）在
  Zig 合成步可能报 `InvalidTag`（如 call 的 reg 操作数越
  界），本端口会成功解码——字节发射面无差（raw_text 不入
  线格式）。符号池由 Zig 的借用切片改 owned `String`，同
  属本条所有权形态。

### SAB-D30 常量兼容检查只比 literal_text（b 类）
- 原文：`ensureConstDeclCompatible` 比 `raw_text` 与
  `literal_text` 双字段。
- 移植形态：本端口只比 `literal_text`。对解码所得常量，
  Zig 的 raw_text 恒为合成式 `@const {name} = {literal}`，
  同名下与 literal 等价 ⟺ raw 等价，判读无差；差异面仅
  「宿主侧以别路径构造、raw_text 与合成式不同」的常量 —
  本端口的常量构造点（C1/C3）均不携带独立 raw_text，该面
  不可达。登记备查。

### SAB-D31 flattenStdSnippet 边界（b 类，结构边界非缺陷）
- 原文：`cachedStdImportModule` / `cachedStdMacroTemplate` /
  `emitStdMacroFragmentFresh` 均经 `flattenStdSnippet`
  （sci SA 文本 flattener + verifier + SAB 编码，宿主侧
  `sci_bridge`）取得片段模块——该机器在 Zig 拓扑里本就不
  在 sab_codegen 内。
- 移植形态：同位置设 `StdSnippetFlattener` trait 边界，机
  器侧把 Zig 逐字构建的 snippet 源码交给 provider；默认
  `NoSnippetFlattener` 响亮报 F 组坐标。片段机器其余部分
  （解码/remap/clone/append/模板/缓存/flush）全部实装并有
  替身 provider 的端到端测试钉死。真 flattener 的移植归
  sci 镜像面后续线，不在本线辖区。

### SAB-D32 缓存片段路径的错误回退收窄（b 类）
- 原文：`emitStdMacroFragmentWithLiteralArgs` 对缓存路径
  的失败只吞 `error.UnsupportedType` 一种（转 fresh 路径），
  其余上抛。
- 移植形态：本端口错误类型无 `UnsupportedType` 对应变体，
  缓存路径错误全部上抛。在无 flattener 的现形态下不可观察
  （两路同死于边界）；候真 flattener 接入时按原文细分。

### SAB-D33 表面注册面：联邦化设计约束登记（蓄意，用户裁定）
- 裁定出处：用户 2026-11-07 14:37（联邦化转入 Rust 侧设
  计约束）+ 14:39/14:40 补正（直引 .sai 保持完全兼容、亦
  为合法生成源）。约束：Rust 侧 SAB 表面注册须「从声明生
  成、可按包联邦」，不扩展 Zig 式手工中央表。
- 本线落实：F 组只**消费**既有规则数据（D1 自
  `std_surface.sla_meta` 解析），未新增任何中央表结构、未
  改注册来源。生成源现状 = .sai/.sla 声明面（含 sla_std
  薄包声明与直引 .sai 声明，两者均合法）；差距仅记「生
  成/联邦机制未覆盖之面」：注册清单的声明生成器与按包合
  并加载面尚未实现（D1 仍读 Zig 时代的中央 meta 文件），
  归后续线（E–I 续线同守本约束）。

## 四期计数增补

- (b) 结构性重构：+4（D29、D30、D31、D32）；蓄意 +1（D33）；
  D26 销账（行为差异消除，条目保留为历史）。
- (c) 新候选：FB-039（atomic_value_ty 存在字节宽松）、
  FB-040（sig 布尔字节宽松与同记录严格校验并存）、
  FB-041（decodeModule 重复段静默覆盖）。
