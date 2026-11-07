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
