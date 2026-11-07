# 分歧登记 — 子系统线 C（sla-checker）

参照源：Zig `src/type_checker.zig`（6,098 行）、`src/control_flow_rules.zig`（276 行），
本机只读仓 `~/workspace/local-sa/sa_plugin_sla/`。
分类：(a) 与 Zig 行为/产出分歧 · (b) Rust 纪律逼出的表达受阻点 · (c) Zig 源码可疑底层问题。
原则：分歧先登记再分类，不许为迁就 Zig 静默改形。本文件所有行号均经本轮源码复核。

## 所有权记账（本线重点）

### D-C1 (b)+(c) 分支/循环状态快照按「名字」键控，遮蔽绑定被合并 ★ 本线头号发现
- Zig 位置：
  - `saveScopeStates`（type_checker.zig:2712）：从最内层作用域向外逐层
    `states.put(name, state)` 写入一张扁平 `StringHashMap(ValueState)`；
    同名键被**外层绑定的状态覆盖**——快照里一个名字只剩最外层那个绑定的状态。
  - `restoreScopeStates`（:2725）：把这一个状态写回**每一层**的同名绑定。
  - if 臂的 then 快照（:5366 起）是同一循环的手工内联复制（没走 saveScopeStates），
    覆盖顺序相同；`mergeMultiBranchScopeStates`（:2738）按名读
    `states.get(name) orelse .active`，同样合并。
  - 连带：cleanups / phi_cleanups 侧表的值是**名字列表**（`[]const u8`），
    释放发射同样按名寻址，遮蔽场景下与状态合并同病。
- 后果（(a) 判定分歧，最小输入，期望值取 Rust 模型判定）：
  ```sla
  fn f(s: String, t: String, flag: bool) {
      {
          let s = t;                 // 内层 s 遮蔽参数 s
          if flag { let a = s; }     // then 分支消费内层 s
          else { }
          let b = s;                 // 内层 s 在 then 路径上已被移动
      }
  }
  ```
  Zig 合并时 then_states.get("s") 取到的是**外层** s 的状态（Active），
  与 else 侧 Active 相等 → keep_current 不动 → 内层 s 的消费丢失，
  `let b = s` 被放行（use-after-move 漏检，源码机制证明；待协调线全管线复现确认）。
  Rust 侧以 `BindingId` 键控快照（scope.rs），内层 s 合并为 Consumed 并报
  UseAfterMove；单测 `shadowing_snapshot_is_per_binding` 钉住 Rust 侧判定。
- Rust 建模：`ScopeStack` 帧内绑定带稳定 `BindingId`，快照 =
  `HashMap<BindingId, ValueState>`；恢复按身份写回，遮蔽绑定互不串扰。
- 处置建议：主会话转主线缺陷登记（检查器名字键控记账类）；Zig 侧修复方向为
  快照/合并/清理三处一并改身份键控，只改一处仍会按名释放错对象。

### D-C7 (b) 消费是「枚举的站点清单」，不是转移规则
- Zig 位置：`consumeBinding` 全部调用点（grep 全量核对）——let 隐式移动
  :2844、解构 :2913、赋值 :2983、release :3129、`^` move 表达式 :3345、
  extern move 实参 :275、await :3620、mem::forget :3903、
  ManuallyDrop::into_inner :3921、Box::into_raw :4068、用户宏 :5127、
  try 传播 :5665。
- 关键事实：**普通函数调用的按值实参在检查器层不被消费**——DEF-5 的
  自动克隆/转移物化在 lowering/codegen（lowering_rules 的
  `planCallArgMaterialization`）决定。于是「调用后值归谁」由两个子系统
  按约定保持一致，检查器的账本本身回答不了这个问题；主线 DEF-5 /
  DEF-deep-copy 家族正出在这条缝上。
- Rust：同一枚举逐站点移植（check_stmt / check_expr / check_call 内逐处
  标注 context 串），并以单测 `plain_call_does_not_consume` 钉住
  「检查器不消费普通实参」这条边界，防止后续移植时顺手「补全」成转移规则。
- 处置：保持枚举形态并在每处标注来源行号；是否升级为统一转移规则属
  语言层裁定，不在本线权限内。

## 诊断通道

### D-C2 (b) 诊断 = 错误联合标签 + 一条 1024 字节定长缓冲
- Zig 位置：`TypeError`（:7）13 个 kind；`last_error_buf: [1024]u8`（:121 附近）
  + `setError`（:234，FixedBufferAllocator，格式化失败降级为固定串
  "Error formatting diagnostic"）；`checkStmt`/`checkExpr` 包装器（:2791/:3143）
  在 last_error 为空时合成兜底文案（"checkStmt failed at node tag {s}" 等）。
- 受阻点：(1) 多个有名字的诊断没有自己的 kind，只能以消息前缀存在——
  `LoopConditionalConsume` 在 `consumeBinding`（:254）以 CompileError +
  文案前缀承载（主线普查的 57 种诊断之一即此形态），`MacroConditionalConsume`
  在宏调用臂（:5127 附近）同理；(2) 裸 `return TypeError.X` 的调用点是否
  有文案，取决于碰巧经过哪个包装器，覆盖随路径变化（见 D-C9）；
  (3) 文案上限 1024 字节，超长静默截断/降级。
- Rust：`Diagnostic { kind, message, marker }` 值返回；kind 与 TypeError
  逐名对齐，命名子诊断进 `marker`（LoopConditionalConsume /
  MacroConditionalConsume / OutOfSlice），不再靠前缀解析。
- 处置：保持；对拍时按 kind+marker 比对，文案只作参考（任务口径）。

### D-C9 (c) while 条件臂有一条无文案的裸错误路径
- Zig 位置：while 臂（:3078-3085）：条件非 primitive 时 setError
  "while condition must be bool/int unless using while let"；
  primitive 但不是 boolean/legacy integer（如 i32）时直接
  `return TypeError.TypeMismatch`——**没有 setError**。用户最终看到什么，
  取决于外层包装器是否合成 "checkStmt failed at node tag while_stmt"
  这类归因错误的兜底文案。
- 附带钉死的行为：while 条件只接受 boolean 与 legacy `integer`，
  有宽度的整数（i32/i64…）作裸条件会被拒。Rust 侧忠实保留并有单测
  `while_sized_int_condition_rejected`；本 crate 该路径给统一文案
  （与 Zig 的文案缺失构成已知的文案级分歧，kind 一致）。
- 处置建议：Zig 侧给该路径补 setError 即可消解，登记为低危诊断面缺口候选。

## 结构性受阻点

### D-C3 (b) 类型图是可变 arena 指针网，侧表按节点指针键控
- Zig：检查过程中 `allocator.create(ast.Type)` 现造类型并原地改写
  （如 call 臂 `ret.* = .{...}`）；`expr_types`/`cleanups` 等五张侧表以
  `*const ast.Node` 为键。后 pass 若重分配/替换节点，旧键静默成孤儿，
  没有任何机制报错。
- Rust：`Type` 为 owned 值（克隆传递）；节点身份 = 借用树内地址
  （`node_key_of`），检查期间 AST 不可变，地址稳定由借用规则**证明**
  而非约定。Zig 里「检查中改写已挂树节点」在本 crate 无对应写法，
  遇之必须重建节点——该 Zig 位置应登记（本切片内未遇到实例）。

### D-C4 (b) 循环检查无不动点 + 循环上下文双通道记账
- Zig：当前循环以两个松散字段 `current_loop_scope` /
  `current_loop_body_terminates` 记账（while 臂 :3086 附近 defer 保存/恢复），
  **同时**又作为 `checkStmt` 的参数 `current_loop_scope` 逐层穿线
  （:2791 签名）——同一事实两条通道。循环体只查一遍（无数据流不动点），
  `consumeBinding` 内的 LoopConditionalConsume 语法禁令是对「第二遍
  迭代会重复消费」的补偿性近似；`bindingIsLocalToLoop`（:112）靠
  作用域父链指针漫步判局部性。
- Rust：单一 `loop_ctx: Vec<LoopCtx>` 栈（栈顶 = 最内层循环，与 Zig
  字段覆写语义等价）；局部性判据 = 绑定所在帧索引与循环帧索引比较，
  与指针漫步判定等价但无链式遍历。禁令与单遍检查均忠实保留——
  近似不在本线权限内改动，特此明示。

### D-C5 (b)+(c) 返回类型借符号表走私（return_ty_sentinel）
- Zig：`checkFunc`（:2573）在函数作用域里定义一个假符号
  `return_ty_sentinel`（is_const=true）承载返回类型；if-let 臂（:5252）、
  switch 臂（:5469 附近）、if 的 else 块检查调用（:5396）等多处以
  `scope.lookup("return_ty_sentinel").?.ty` 读回。
- (c) 面：`.?` 是 panic 不是诊断——「进入这些臂之前 sentinel 必然已定义」
  是无文档的调用顺序约定，任何新臂漏守约定即编译器崩溃而非报错；
  且该符号污染一切作用域遍历，清理/合并处处要 `isInternalSymbol` 特判
  （lowering_rules:4066，该名字是唯一的内部符号）。
- Rust：`Checker::current_ret_ty: Option<Type>` 普通字段；无符号污染，
  无 unwrap 路径。注入绑定用的名字判定 `is_internal_symbol` 仅作
  兼容保留。

### D-C6 (c) 块退出清理名单的顺序 = 哈希迭代序（可疑，未证实）
- Zig：`checkBlock`（:2618）退出时遍历 `scope.symbols.valueIterator()`
  （StringHashMap 迭代序）生成 cleanups 名单。若 codegen 按名单顺序
  发射释放，释放顺序随哈希布局而非声明顺序变化；嵌套所有权（如
  先建后拆的字段关系）下顺序可能有语义。**顺序敏感性未实证**，
  登记为可疑候选，候主线在 codegen 侧核对 drop 顺序是否被依赖。
- Rust：帧内绑定保声明序（Vec），清理名单按声明序生成。

### D-C8 (b) 两套类型相等并存，严格度按调用点手工选
- Zig：`typesEqual`（:5752，源码复核）——跨 union 标签永不相等；
  primitive 先精确后按整数类/浮点类互等（**宽度不敏感**，i32==i64）；
  array 连长度一并比；fn_ptr 连 abi 串一并比；infer 通配。
  `typesStructurallyEqual`（:376）primitive 严格相等。
  调用实参处对 array 元素另有宽度**严格**的 `arrayCallArgStrictMatches`
  （注释自承为防「静默整数宽度不匹配」的 #1 号修复）——严格只打在
  出过事的那一处，与默认宽松口径并存、靠调用点自觉。
- Rust：两套相等按原名移植（`types_equal` / `types_structurally_equal`），
  array 实参严格规则落在 check_call 对应位置；单测
  `width_insensitive_let_passes` 钉住默认宽松口径，防误「修正」。

### D-C10 (a) FFI 实参类型整体旁路（行为保留，显式登记）
- Zig：extern 调用臂（:5072）：实参表达式照常检查，但类型被丢弃
  （源码注释 `// bypass signature checks for FFI pointers`），只查
  arity、`^` move 契约与 ABI 返回串。`extern_funcs` 仅由 .sai 合同
  文件解析填充（:2106），不来自程序内的 extern 声明。
- Rust：逐字保留，经 `register_extern_func` 注册；单测
  `extern_bypass_ignores_arg_types` / `extern_move_contract_consumes`
  钉住。登记备查：这是有意设计而非缺陷，但差分实验的价值正在于
  把这类「检查器故意不看」的面显式化。

## 覆盖与验证

- 移植映射：`control_flow_rules.zig` 全量 → rules.rs（分支合并计划、
  函数出口清理计划、多路合并交集、宏参数消费格与 AST 漫步器）；
  `type_checker.zig` 切片 → checker.rs（注册表/consumeBinding 门/
  typeIsCopy）、check_program.rs（注册遍/checkFunc/checkTest/checkBlock/
  终结判定）、check_stmt.rs（let/const/var/assign/return/for/while/
  break/continue/release）、check_flow.rs（if 表达式与 keep_current
  合并 + phi 清理）、check_expr.rs（字面量/标识符/二元/borrow/move/
  deref/cast/field/结构体字面量/元组/数组/index/try/await/unsafe）、
  check_call.rs（sum 特例 → extern 旁路 → 内建 → 宏 → 用户函数 →
  构造器/fn-ptr 的分派序，及 mem::forget / Box::into_raw /
  ManuallyDrop::into_inner 三个关联消费特例）。
- 消费站点覆盖（对照 D-C7 清单）：let/赋值/release/move/extern/宏/
  try/await 八处 + 关联特例三处已移植；~~仅解构移动（:2913）随
  let_destructure 一并未覆盖~~（二期已补齐，见文末二期销账节）。
- 构建：Kaggle 场地（2201，cargo/rustc 1.99.1）`cargo test -p sla-checker`
  全绿（16 测试，含 D-C1 遮蔽演示、D-C8 宽度口径、D-C9 while 口径、
  双 extern 行为、宏条件消费、循环条件消费五项钉死测试），
  干净重编零警告；日志 `docs/build-logs/checker-build-20261107.log`。
  每个 .rs ≤700 行（最大 tests.rs 581 行）。
- 未覆盖清单（切片外，遇之以 OutOfSlice 响亮拒绝，不静默近似；
  二期已全部或大部销账，逐项状态见文末二期销账节，此处保留原文作对照）：
  switch/match 臂（含 mergeMultiBranchScopeStates 的应用层——规则已移植）、
  if-let 链、let_else / let_destructure、闭包字面量与立即调用、
  泛型函数实例化检查、trait/impl 契约检查（方法体已查）、
  运算符重载与数值结构体二元特例、dyn trait / slice 强转（let 处）、
  await_cleanups 与 try_cleanups 与 macro_call_try_cleanups 三张侧表、
  导入宏（imported_macros）、enum 字面量定型、slice 表达式、
  sum 的 slice 形态、关联调用的完整解析（现为限定名候选近似）。
- 接线说明（交协调线）：本 crate 直接消费 sla-ast（根 Cargo.toml 的
  path 依赖为协调线所建，未改动）；任务书预设的「临时镜像」因 sla-ast
  已就绪而未启用（镜像文件已删除）；未改任何根 workspace 文件。

## INT-1 销账（2026-11-07，二期 checker 线第一段）

### INT-1 (a) 本移植缺陷 — 已修复销账：非 void 函数对在场 return 误报 missing return
- 根因（一句话）：Rust `check_tail_type` 自行发明了 Zig 不存在的
  「missing return」判定——它只认尾 `expr_stmt` 为函数返回值，尾随
  `return_stmt` 时 `block_tail_type` 返回 None 即报
  "missing return value in non-void function"；而 Zig `checkFunc`
  （type_checker.zig:2596-2604）**根本没有 missing-return 检查**：
  仅当 ret 非 void、body 非空、末语句为 `expr_stmt` 且该语句不终止
  时才核对尾表达式类型，其余形态（return 尾、终止性尾表达式、
  非表达式尾、空 body）一概不查；return 值本身已由 `return_stmt`
  臂（:2991 起）核对。
- 修复：`crates/sla-checker/src/check_program.rs` 的
  `check_tail_type` 按上述 Zig 原形逐条件重写（void/空 body/
  非 expr_stmt 尾/终止性尾 → 直接通过；非终止 expr_stmt 尾 →
  取 `expr_types` 核对，缺记录时 CompileError 与 Zig
  `orelse return TypeError.CompileError` 对齐，不匹配改为
  TypeMismatch 且文案与 Zig 同式
  "TypeMismatch in function tail expression: expected …, actual …"，
  此前 Rust 侧误用 CompileError）。发明的 missing-return 错误已删除。
- 附带实证的 Zig 行为（忠实保留，不作 (c) 另立）：非 void 函数以
  非表达式语句结尾（`let` 尾）或空 body 时，Zig 检查器同样放行——
  这不是本次修复引入的宽松，而是 Zig 检查器口径本身；已由单测
  `non_expression_tail_passes_per_zig` / `empty_body_passes_per_zig`
  钉住，防止后人再以「补全 missing return」之名复活 INT-1。
  （分类说明：本条按本文件分类法属 (a) 与 Zig 行为分歧之本移植
  缺陷；任务书预置的 (b) 标签经实证不适用——(b) 指 Rust 纪律
  逼出的表达受阻，本缺陷与之无关。）
- 真值依据（双路）：① Zig 源码 type_checker.zig:2596-2604 +
  :2991；② 场地三真插件（现头 6002b386，`sa sla check`，2026-11-07）
  九探针实测：return 尾 rc=0、if 双臂 return rc=0、尾 if 表达式
  rc=0、尾算术表达式 rc=0、尾类型不匹配 rc=1 (TypeMismatch in
  function tail expression: expected i32, actual boolean)、
  let 尾 rc=0、return 类型不匹配 rc=1 (TypeMismatch in return)、
  空 body rc=0。
- 新增单测（`crates/sla-checker/src/tests_e2e.rs`，parse→check
  形态，经 Cargo dev-dependency 引 sla-parser；另 `src/lib.rs`
  注册模块）：e2e_c 本体（与 docs/probes/e2e_c.sla 逐字一致）+
  if 双臂 return、尾 if 表达式、尾算术表达式、尾类型不匹配、
  return 类型不匹配、let 尾、空 body，共 8 条。
- 场地实测（场地三 2202，md5 双确认 0 差异后）：
  `cargo test -p sla-checker` 24/24 全绿（原 16 + 新增 8）；
  `cargo test --workspace --no-fail-fast` 72/72 全绿（基线 64 + 8，
  无回退）；oracle 全管线对 e2e_a/e2e_b/e2e_c 三探针均 rc=0
  （修复前三者皆死于 INT-1）。

## 二期扩围销账（2026-11-07，checker 线第二段）

按任务书顺序逐项镜像（Zig 行段为本机参照树 type_checker.zig 口径），
场地三（2202）实测：`cargo test -p sla-checker` 80/80
（起点 24，新增 56）；`cargo test --workspace --no-fail-fast`
134/134（起点 72，只增不减）。真值双路：Zig 源码 + 场地真插件
（现头 6002b386）12 探针（switch 消费/switch 尾/if-let/let-else
正反/destructure Copy/match 正反/泛型/slice 正反/闭包推断/
while-let/await 重复）逐项对齐。

### 逐项销账

1. **switch/match 臂应用层**（switch :5464、match :5494）：
   `check_match.rs` 新建。switch 只查 scrutinee 与臂体、模式不查
   （D-C13）；match 三路（infer 推断路 / Option·Result 路 / 枚举路）
   全量移植含守卫检查与活臂尾类型一致性；多路合并走
   `merge_multi_branch_states`（BindingId 键控，D-C1 的 Rust 侧
   纪律在新代码同样适用）。`expr_terminates` 补 match
   （非空且全臂终止）与 switch（再加 default 臂存在）形态。
   sentinel 读回点一律以 `current_ret_ty` 字段对应（D-C5 纪律）。
2. **if-let 链**（:5252）：`check_flow.rs::check_if_let`，链条件在
   chain 帧内顺序检查、后条件可见先绑定；合并与 plain if 同构。
3. **let_destructure + 消费站点 :2913 与 let_else**（:2858/:2866）：
   `check_stmt.rs`。解构根无条件消费（D-C12/FB-010）；let-else 的
   else 块先于绑定检查且必须发散。
4. **泛型函数实例化检查**：检查器层**不存在**该检查——实例化归
   monomorphizer（D-C11）；`generic_func_ref` 臂裸 CompileError
   照录（D-C14/FB-012）。
5. **三张侧表**：`await_cleanups`（:336 recordAwaitCleanup，含逃逸
   与借用别名排除）、`macro_call_try_cleanups`（:5095，传播参数
   排除 + 递归 macroHasTry/macroParamIsTrySource，check_macro.rs）；
   **try 无独立侧表**——Zig 的 try 清理写入共享 `cleanups`
   （collectTryCleanup :1662），任务书「try_cleanups」即此，已照
   Zig 结构合并登记（D-C17）。
6. **清单余项**：enum 字面量定型（check_literal.rs，:4780）；
   slice 表达式（同文件，:4734，字面量界 + 静态界检查）；
   导入宏（check_macro.rs 名称结果表全量移植 lowering_rules:3020
   + 调用臂 arity/表达式输出双形态）；trait/impl 契约
   （check_trait.rs：超 trait 须直接 impl、方法须为 trait 自有、
   Self 替换签名相等）；运算符重载 + 数值结构体二元特例
   （check_binary.rs：overloadSymbol 精确匹配 + AmbiguousCall、
   struct 加减/标量乘特例、Eq/Ord 派生门）；dyn/slice 强转
   （check_stmt.rs let/const 臂 + dyn_box/dyn_rc_coercions 侧表，
   附带修正 const 臂误消费——Zig const 臂从不消费初值源）；
   闭包字面量（推断参数响亮拒绝，文案与场地探针逐字一致）+
   IIFE + 局部闭包/fn-ptr 变量调用（先于用户函数解析）；
   关联调用补 trait 方法符号 `{Type}__{Trait}_{method}` 候选；
   sum 的 slice 形态（收紧为 array/slice/vec 三路，删除一期
   「任意泛型首参」近似）。

### 二期新登记分歧

- **D-C11 (b) 泛型实例化不在检查器**：Zig 流水线 monomorphize →
  type-check，检查器从不见未特化泛型函数；`FuncDecl.generics`
  在 type_checker.zig 全文未被查阅，调用点的 `CallExpr.generics`
  仅内建特例使用。Rust 镜像照此：登记泛型名但从不替换，用户
  调用按未替换签名检查。形态差异属流水线分工而非检查器行为差。
- **D-C12 (b) 解构消费无 Copy 豁免**（→ FB-010）：:2913 无条件
  consumeBinding，与相邻 let 站点（:2844）的 Copy/borrow 豁免
  口径分叉；场地实证 Copy 元组解构后复用 rc=1 UseAfterMove。
- **D-C13 (b) switch 模式零检查**（→ FB-011）：:5464 臂从不查
  case.pattern——无定型、无绑名、无穷尽性。
- **D-C14 (c) generic_func_ref 裸错误**（→ FB-012）：:3209
  `return TypeError.CompileError` 无 setError，诊断只能靠包裹层
  回退文案。
- **D-C15 (b) pattern writable→is_const 反转**（→ FB-013）：
  definePatternBindings（:1649）把名为 writable 的参数直塞
  defineSymbol 的 is_const 槽；if-let/while-let/match 传 true
  （绑定实为 const）、let-else 传 false（实为可变）。Rust 照录。
- **D-C16 (b) match 推断路 OR 条件 + 改写 scrutinee 类型**
  （→ FB-014）：:5494 推断路以「枚举名或变体名」析取判定
  Option/Result，并回写根绑定 `sym.ty`。
- **D-C17 (b) 侧表键与清理名单的寻址口径**：Zig
  macro_call_try_cleanups 按 `*const CallExpr` 子对象指针键控，
  Rust 按包含节点地址键控（节点身份口径差，同 D-C3 家族）；
  三张侧表的值在 Zig 与 Rust 两侧同为**名字列表**（身份键控只
  覆盖状态快照/合并，清理名单的按名寻址是 Zig 原结构，照录并
  在此明示其与 D-CK1 的关联面）。

### 二期剩余未覆盖清单（确切）

- 关联调用的**接收者类型推导**全形态：现实现覆盖显式 target 的
  限定名候选 + trait 方法符号扫描；由首参表达式类型反推接收者
  类型的形态仍为近似（Zig 侧该路径与方法调用下沉形态耦合，
  本段未逐行移植）。
- match 守卫（guard）的完整形态：已移植「守卫须为基本
  boolean/integer」校验；守卫内的绑定可见性与消费语义未单独
  建模（Zig 侧守卫与 case 体同帧，本镜像同帧处理，作为近似
  登记在此）。
- plain if 条件接受 integer 一项：一期镜像要求 Boolean，
  Zig 接受 boolean/integer/infer——属一期既有简化，本段
  按任务范围未动，登记在此待后续线处理。

## 三期销账（2026-11-07，CHK3 线）

二期剩余 3 项逐项攻下。真值口径说明（重要）：本机参照树为
3de91932（type_checker.zig 6,098 行），场地现头 6002b386 的
同文件为 10,917 行——两树同机器、行号不同；本段行号一律以
**场地现头**为准，探针（31 件，`sa sla check`，rc + 诊断原文）
为最终真值。场地实测：`cargo test -p sla-checker` **118/118**
（80 起点，新增 38：tests_phase3.rs 18 + tests_phase3b.rs 17 +
tests_phase3c.rs 3）；workspace 全量除 sla-plugin 一件
INT-2 线测试的文案 pin 外全绿（见 D-C18 跨线注记）。

### 逐项销账

1. **关联调用接收者推导全形态**（新文件 check_method.rs）：
   逐形态镜像——① 关联 trait 形 `Trait::m(recv, ...)`（:8438，
   args[0] 具体类型选 impl、declaring trait 命名符号、funcs-only
   取符号）；② 关联固有形 `{Target}_{m}`（:8787，funcs-only）+
   `Option::None` 构造 + 关联 trait 回退（:8796，funcs-only）；
   ③ 接收者推断固有形（:9669，peel borrow/pointer → `{T}_{m}`，
   funcs + imported、auto_borrow_receiver=true）；④ 接收者推断
   trait 形（:9688，`resolveTraitMethodSymbolForType` 全量移植：
   target-trait 支 + 全 impl 扫描支、多符号歧义 = TypeMismatch
   且文案逐字）；⑤ dyn 接收者（`__dyn_` 载体 → trait 声明方法、
   params[1..] 核）；⑥ `using` 静态扩展（:9703-9725 + resolver
   :2922 + checkUsingExtensionCall :2978：固有 → trait → using
   的 DEF-E0 定序、仅 pub、extensionReceiverParamMatches 含
   借用型首参、歧义 = AmbiguousCall 且文案逐字、接收者名
   primitive 探针形态）。方法路径共用实参检查器
   `check_method_args` = `checkCallArgsAgainstSignature` 忠实
   移植（auto-borrow 支、借用型参数支、capability 门、
   plainCallArgMatches、数组宽度严格规则）。关键探针真值：
   `&T`/`*T` 类型接收者调 plain-self 方法 rc=1 TypeMismatch
   （peel 只定 key、不定实参——标识符的借用类型不解包、只有
   借用表达式解包）；固有优先于 trait（返回类型探针判别）；
   关联 miss 回落到后续解析级（与接收者推断级衔接，imported
   静态形因此不可达而点调用形可达——非对称性已钉测试）。
   using 状态基建：Frame 加 using_modules（规范化路径）、
   checkProgram 全局登记 + checkStmt 作用域登记、
   FuncSig 加 is_pub。
2. **match 守卫绑定可见性/消费语义**：源码 + 10 探针核清后
   判定——二期「同帧近似」在状态语义上与 Zig **结构等价**
   （Zig pattern_scope 为 case 体块作用域之父、Rust 图案帧
   同位），无代码改动、以探针形态单测钉死：守卫见图案绑定
   rc=0；守卫 `^v` 消费图案绑定 → 体内复用 rc=1 UseAfterMove
   （Option 与 enum 两路）；守卫 try 消费外层绑定 → 体内复用
   rc=1、且消费进入 case 活状态经多路合并使 match 后复用
   rc=1（**守卫假路径无回滚**——检查器根本不建模守卫失败
   分支，守卫与体同序检查、效应同账）；跨 case 绑定不可见
   rc=1 UndefinedVariable；图案绑定不逃逸 match rc=1；
   守卫类型 boolean/legacy-integer 之外（i32、整数字面量）
   rc=1 TypeMismatch。
3. **plain-if 条件**：check_flow.rs 改为 Zig 原形三形态
   （boolean / legacy integer / infer 通过，其余 TypeMismatch
   无 setError）。探针附带定性：源级 `int`/`integer` 注解与
   整数字面量在现头检查器里都**不**产出 legacy integer 类型
   （rc=1），legacy integer 实际只由比较表达式与宏参数
   （infer）到达——注解层事实、非检查器分歧，登记备查。

### 三期新登记分歧

- **D-C18 (a) 未解析调用的回退诊断已对齐**（本移植缺陷，已修）：
  原镜像对一切未解析调用合成 CompileError「checkExpr failed
  at call …」；Zig 调用臂底回退（:9982）对 plain/点/关联三形
  一律 setError「Undefined call: {name}」+ UndefinedVariable
  （三形探针实证）。已改齐。**跨线注记**：sla-plugin 的 INT-2
  线测试 `println_remains_blocked_by_missing_checker_builtin_arm`
  的文案 pin 钉的正是旧回退串，本项落地后该 pin 需由该线改为
  「Undefined call: println」（其意图——println 仍被阻断——
  不变，仍是错误）；本线辖区外，未动其文件，候协调线转达。
- **D-C19 (a) 接收者级在分派序中的位置差**（剩余近似）：
  Zig 接收者级位于 plain 函数/构造器之后、extern/宏之前；
  Rust 分派序中 extern/宏在前、接收者级殿后。仅当同一调用名
  同时命中宏/extern 与方法形时判读不同（名字碰撞面），
  已登记候后续线按 Zig 序重排分派时一并处理。
- **D-C20 (b) FuncSig 不载 is_async**：Zig 方法/关联路径对
  async 被调用方把返回型包成 future；Rust FuncSig 无此字段，
  方法路径不包装（与 plain 调用路径的既有同一缺口，非本段
  新增）。候签名面统一补 is_async 时一并销。
- **D-C21 (b) 方法路径的侧表与强转缺口**：check_method_args
  未 port 的 Zig 侧表/谓词三件——dyn_borrow_args 侧表（判定
  已 port、记录缺）、array_to_slice_borrow_args 及其
  borrow-array→slice 强转（方法实参处不识别）、
  `ptr::read_volatile`（本 crate 无 unsafe_depth 计数器，
  关联形回落到未解析）。均为判定外围/记录面，不影响已钉
  探针的 rc 口径，逐件登记在此。
  **【四期注记 2026-11-07】**：第三件已随 D-C22 线销账——
  `unsafe_depth` 计数器已入 checker（UnsafeExpr 臂维护），
  关联形与裸名形 read_volatile 均按现头口径镜像；余两件
  （dyn_borrow_args 记录面、array_to_slice 强转）仍在册。
- 观察（不立单）：match 守卫的 TypeMismatch 在 Zig 无
  setError、靠包裹层合成「checkExpr failed at node tag
  match_expr」——与 D-C9 同族的诊断面缺口，既有登记覆盖。

### 三期剩余未覆盖清单（确切）

- 接收者形状键控的 std 叶级联（Option/Result/Vec/HashMap/
  BTree/Cell/Atomic 等 `isXCall` 方法叶 + push/pop/remove +
  iter/copied/collect/join/map/filter/fold 叶）：按名字键控的
  std 合同叶、非接收者推导机器本体；sum 叶已在一期 port。
- 静态构造叶（`Vec::new`/`HashMap::new`/`VecDeque::from` 等
  关联形）与 `str::from_utf8` 叶：同上，std 表面叶。
- plain 调用路径的实参检查仍为一期简化形
  （check_call_args_against_sig）；忠实形只落方法路径
  （check_method_args）——两形并存的收敛候后续线。
- D-C19 / D-C20 / D-C21 三条（见上）。

### 反馈单

本段无新 Zig 侧缺陷候选：三项的 Zig 行为经探针核清后均
自洽（守卫无回滚属检查器单遍模型的既定近似、与 D-C4 同族
已在册）。号段 FB-029..FB-032 未使用，归还协调线。

## 三期 INT21 销账（2026-11-07，INT21 补臂线）

INT2-1 落地：场地现头（6002b386，type_checker.zig 10,917 行）
`call_expr` 臂中位于关联块之后、用户函数解析之前的裸名特判块
（:8805-8895）全量镜像于新文件 `check_builtin.rs`，`check_call`
在关联块后、其余解析级前接入（与 Zig 分派序一致，不可遮蔽 —
场地探针：用户 `fn println(x: i32)` / `fn len(a, b)` 均不能
改变臂判定）。逐臂：`vec`（≥1 参、全参 typesEqual、→Vec<T>、
0 参 setError 原文）、`len`（恰 1 参、**参数类型不查** —
`len(42)` rc=0、→usize、arity 错带 setError 原文）、`str_eq`
（恰 2 参、双 string-like — lowering_rules:5201：raw_ptr/
array/String peel 后判定、字面量与数组均合格、→boolean、
实参不消费）、`format`（首参须字符串**字面量节点**、变量
格式串 rc=1 InvalidArgsCount、→String）、`hash`（恰 1 参、
typeIsHash 门、→u64）、`debug`（恰 1 参、typeIsDebug 门、
→String）、`println`（任意元数含 0、实参只查不消费、→void）、
`std__panic__catch_unwind` / `panic::catch_unwind`（恰 1 参、
0 参闭包且字面量体为直接 panic 调用、否则 setError 原文、
→Result<i32,i32>）。同块的 `panic` 臂不重复镜像（本 crate
既有 builtin 步同判读）。typeIsHash/typeIsDebug 按
type_checker.zig:4807/4819 + lowering_rules 基（hashable：
primitive 除 raw_ptr/void/浮点；debug 基 = Copy-value：
除 raw_ptr/void、**含浮点** — 探针 debug(1.5) rc=0 /
hash(1.5) rc=1）+ std owner 名单 base=false（String 值
hash/debug 均 rc=1）+ struct derive 门（case-insensitive、
非 opaque/非 union、字段递归 — String 字段毒化 hash）
全量移植；Zig 字段递归无界，本移植随 check_binary 的
type_is_eq_ord 先例带 visiting 集（自引用 derive 结构体
判 false 而非无限递归）。场地探针 33 件（rc + 诊断原文）
逐臂钉死，单测 tests_phase3d.rs 28 条（含 parse→check
全管线 + 五 fixtures 本体逐字）。

### INT21 后新登记分歧

- **D-C22 (a) 关联块之前的裸名特判普查余项**：同一 `call_expr`
  臂在关联块之前还有一段裸名特判（:8212-8436）：`Some`/`None`/
  `Ok`/`Err` 构造形、`monitor`、`ptr__null`、
  `future__*` / `poll__*` / `RAW_WAKER_*` / `WAKER_*` 运行时叶、
  `va_next_*`、`ptr__read_volatile`。本线未镜像（INT2-1 定层
  与 INT-2 §7 的补臂清单均只含关联块之后一段；`None` 标识符
  形与 `Option::None` 关联形已在册）。登记为覆盖缺口候四期：
  阻塞条件 = 语料/fixture 出现这些名字的**调用形**且 Rust
  侧落到 Undefined call 回退（现 fixtures 与 e2e 均未触达 —
  Option/Result 构造在现有管线走 enum 字面量/模式路）。
  **【四期 D-C22 线销账 2026-11-07】**：全段已镜像于新文件
  `check_preassoc.rs`，接入点按 Zig 原序置于关联块**之前**
  （`check_call`）。真值 = 场地现头 sla `2cc12ea8`
  （type_checker.zig 11,022 行，本段现址 :8320-8541）真插件
  90 探针（`docs/probes/dc22/`，rc + 诊断原文）；单测
  tests_phase3e.rs 29 条逐名钉住，checker 175/175、workspace
  323/0（基线 294 + 29，零回退）。逐名实证结论：

  | 名 | 实证行为（rc 口径） |
  |---|---|
  | `Some(x)` | 恰 1 参 → Option<inner>；arity≠1 InvalidArgsCount（无 setError）；用户同名 fn 不可遮蔽（2 参 fn 在场时 Some(1,2) 仍被臂拒）；**谓词不查 associated_target**（`foo::Some(42)` rc=0，见 FB-053） |
  | `None()` | 仅裸调用；0 参 → Option<infer>；None(1) InvalidArgsCount。标识符形 `None` 走既有字面量路（rc=0） |
  | `monitor(h)` | 仅裸调用；arity≠1 CompileError（setError 原文）；实参 peel 后须 primitive u64 否则 CompileError（setError 原文）；`monitor(&h)` rc=0；记录 monitor_calls 侧表（本线新携）；→ Chan<(u64, i32)>（return 位错配报 TypeMismatch "actual Chan"）；`foo::monitor(h)` 回落 Undefined call |
  | `Ok(x)` / `Err(x)` | 恰 1 参 → Result<inner, infer> / Result<infer, inner>；arity InvalidArgsCount；同样不查目标（`foo::Ok(42)` rc=0，FB-053） |
  | `ptr__null` / `std__ptr__null` | 0 参 + 恰 1 泛型 → *T；无泛型 InvalidArgsCount。**拼写事实**：裸名后 `::<T>` 是 Zig 解析器 SyntaxError（探针 p_ptrnull_ok 等 rc=1 系语法层），可达拼写是 `ptr__null<T>()`（r_ptrnull_plain_generic rc=0）；关联形 `ptr::null::<T>()` 为既有路径 |
  | `future__pending` | 0 参 + 1 泛型 → future<T>（泛型同样走 `<T>` 拼写）；无泛型 InvalidArgsCount |
  | `future__defer_ready(x)` | 1 参 + 0 泛型 → future<inner>；带泛型在 Zig 为 SyntaxError（解析层） |
  | `future__join2` / `future__select2` | 2 参 + 0 泛型；两参须 future 且载荷 poll-scalar（= numeric，含浮点）否则 TypeMismatch；→ future<FuturePair<L,R>> / future<FutureEither<L,R>>（return 位实证） |
  | `future__pair_left/right` | 1 参 FuturePair；选侧须 poll-scalar 否则 TypeMismatch（String 侧实证） |
  | `future__either_side` | 1 参 FutureEither → u64；非 either TypeMismatch |
  | `future__either_left/right` | 同 pair 访问器（either 载体） |
  | `poll__ready(x)` | 载荷须 numeric（含 1.5 浮点 rc=0；String rc=1 TypeMismatch）→ Poll<inner> |
  | `poll__pending` | 0 参 + 1 泛型，泛型须 numeric（String 泛型 TypeMismatch）→ Poll<T> |
  | `poll__is_ready/is_pending` | 1 参须 Poll → bool；非 Poll TypeMismatch（**不查**载荷 scalar 性） |
  | `poll__value` | 1 参 Poll → inner，inner 须 numeric（Poll<String> TypeMismatch） |
  | `RAW_WAKER_NEW(d, v)` | 2 参均须 pointer 值（*T 或 raw_ptr）否则 TypeMismatch → RawWaker |
  | `RAW_WAKER_GET_DATA/GET_VTABLE` | 1 参（类型不查）→ *infer |
  | `RAW_WAKER_CLONE` | 1 参 → RawWaker |
  | `WAKER_FROM_RAW` / `WAKER_CLONE` | 1 参 → Waker |
  | `WAKER_GET_DATA/GET_VTABLE`、`LOCAL_WAKER_GET_DATA/GET_VTABLE` | 1 参 → *infer |
  | `WAKER_WILL_WAKE` | 2 参 → bool；arity≠2 InvalidArgsCount |
  | `WAKER_WAKE/WAKE_BY_REF`、`RAW_WAKER_WAKE/WAKE_BY_REF` | 1 参 → i32（return 位实证） |
  | `va_next_i32/i64/u32/u64/f64/ptr` | 门序 = 变参门 → arity → 标识符 → VaList 类型：非变参定义内 TypeMismatch（setError 原文）；变参内 arity≠1 InvalidArgsCount、非标识符实参 TypeMismatch（setError）、非 VaList 绑定 TypeMismatch（setError 带实参类型）；未知 `va_next_*` 前缀 UndefinedVariable（setError 支持列表原文）。成功路（rc=0）经语料形态实证（va_sum/va_mix/va_first_ptr 探针），但 Rust 侧被 ast 面挡住，见 D-C24 |
  | `ptr__read_volatile` / `std__ptr__read_volatile` | unsafe 外 CompileError（setError "ptr::read_volatile requires unsafe"）；unsafe 内 1 参须 pointer/borrow 否则 TypeMismatch → pointee。本线同时补齐 D-C21 所缺的 `unsafe_depth` 计数器（check_expr 的 UnsafeExpr 臂增减），关联形 `ptr::read_volatile`（现头 :8568）同门接入关联块 |

  附带定性（不立单）：本段绝大多数 arity/kind 错误**无
  setError**，用户所见文案由包裹层合成（"checkExpr failed
  at call {name} with {n} args (error.{kind})"）——与 D-C9
  同族诊断面事实，既有登记覆盖，不重复立单。
- **D-C23 (a) 02/07 全管线残差 = D-C6 的端到端实形**：补臂后
  五 fixtures 经 oracle（build 模式）全 rc=0，01/03/04 与
  expected **逐字节全等**；02_arith 与 07_functions 各只剩
  **一处清理序对位差**：函数出口清理名单中首对参数的释放序
  与 Zig 相反（07 `add`：Rust `!a !b` vs Zig `!b !a`；02
  `arith_values`：Rust `!a !b !c !d` vs Zig `!b !a !c !d`），
  字节数与行多重集完全相同。即 D-C6（Zig `checkBlock` 出口
  遍历 StringHashMap valueIterator 的哈希序）在全管线上的
  首个实证：Zig 序既非声明序也非逆序（四绑定时仅首对交换），
  是哈希布局序。处置候协调线裁定：复刻哈希序须模拟 Zig
  StringHashMap 布局（跨构建稳定性未证），或推动 Zig 侧改
  声明序（与 FB 方向一致）；本线不以「交换首对」式近似迁就
  （名字集变化即失效，且属为对拍改形）。归 codegen/checker
  清理序议题，非 INT2-1 残留。
- 观察（不立单）：`tests.rs::imported_println_in_test_block`
  的注释仍称 println 来自 imported 签名表；补臂后该测试经
  println 臂通过（断言不变、仍绿）——归因注释候 checker 线
  后续顺手更正，不影响判读。

### 附带项裁定（INT2-2 / INT2-3，详见 contracts.md 销账注记）

- INT2-2（ExternSig 缺 return_fallible）：**不阻塞**五 fixtures
  后段 — Zig 侧读者只有 codegen.zig:2989/19043 与
  sab_codegen.zig（均读 extern 条目作代码生成），检查时行为
  不读；五 fixtures 源码不直调任何 fallible extern（其调用
  只有 println/str_eq），codegen 对 sa_* 的发射走 lowering
  规则而非 checker extern 表。字段补齐本身只差一行，但构造点
  在辖区外（sla-plugin/src/contracts.rs 的 `extern_sig_of` 与
  sla-oracle/src/main.rs 的 shim 字面量）——在本线单方面加
  字段会使两 crate 编译失败、workspace 倒退，故登记精确
  阻塞条件候协调线原子落地：当任一 fixture/语料出现「源码
  直调 fallible extern 且须经 checker ExternSig 把 fallible
  传到 codegen」时触发。
- INT2-3（checker 无 layout_defines 表）：**不阻塞任何段** —
  场地现头全仓 grep 实证：`layout_defines` 在 Zig 侧只有
  写入（loadContracts，type_checker.zig:6064）与 deinit，
  **没有任何读者**（checker/lowering/codegen 全无 get）；
  它是惰性登记面，缺席不改变任何判读。Rust 侧 323 条 defines
  已由 LoadReport 携带可见。不为补而补：候 Zig 侧出现首个
  读者时再镜像登记面。

### 反馈单

本线无新 Zig 侧缺陷候选：逐臂行为经探针核清后均自洽
（len 不查参数类型、debug 含浮点而 hash 不含、String 不可
hash/debug 等均为一贯设计口径）；D-C23 的哈希序清理属
D-C6 已在册候选的实形确认，不重复立单。号段 FB-029..
FB-032 未使用，归还协调线。

## 四期销账（2026-11-07，D-C22 线）

D-C22 全段销账（详见上文 INT21 节 D-C22 条的四期注记表）：
新文件 `check_preassoc.rs`（关联块前裸名段全量镜像，接入序
与 Zig 一致），checker 新携 `monitor_calls` 侧表、
`unsafe_depth` 计数器、`current_fn_is_variadic` 门状态；
tests_phase3e.rs 29 条单测与 90 件真插件探针逐名对应。
场地实测（Colab 盒工作副本，HOME=/content/plhome-rust4）：
`cargo test -p sla-checker` 175/175（146 基线 + 29）；
`cargo test --workspace --no-fail-fast` 323/0（294 基线
+ 29，零回退）；check_file_lines.sh 通过（131 个 .rs）。

### 四期新登记分歧

- **D-C24 (b) va_next 成功路被 sla-ast 表面挡住**：Zig 的
  `va_next_*` 成功路要求 (1) FuncDecl 的变参定义标记
  （`current_fn_is_variadic_def`，由 `extern "C" fn f(...
  args: ...)` 定义置位）与 (2) primitive `va_list` 类型的
  实参绑定。Rust 侧 sla-ast 的 FuncDecl 无变参标记、
  Primitive 无 VaList 变体、sla-parser 不识别 `args: ...`
  形参——三处都在本线辖区（sla-checker）之外。现镜像：
  `current_fn_is_variadic` 字段已携（默认 false，解析程序
  恒非变参，与 Zig 对一切非变参上下文的判读一致），门序与
  全部错误路（变参门/arity/标识符/VaList 类型，含 setError
  原文）已按探针钉死（tests_phase3e 的 variadic 驱动测试
  直接置位门状态）；成功路的 read-type 映射已备
  （va_next_ptr → raw_ptr 等六名），候 ast/parser 表面补齐
  后把「类型 == VaList → 返回 read type」一段接上即全销。
  阻塞条件（精确）：sla-ast 增 VaList primitive 与 FuncDecl
  变参标记、sla-parser 识别 `...` 形参，三者任一落地时由
  协调线定序补接。
- 反馈候选：本段新发 **FB-053**（构造四谓词不查
  associated_target，见 docs/feedback/）；号段 FB-054、
  FB-055 未使用，归还协调线。
