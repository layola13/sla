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
  try/await 八处 + 关联特例三处已移植；仅解构移动（:2913）随
  let_destructure 一并未覆盖。
- 构建：Kaggle 场地（2201，cargo/rustc 1.99.1）`cargo test -p sla-checker`
  全绿（16 测试，含 D-C1 遮蔽演示、D-C8 宽度口径、D-C9 while 口径、
  双 extern 行为、宏条件消费、循环条件消费五项钉死测试），
  干净重编零警告；日志 `docs/build-logs/checker-build-20261107.log`。
  每个 .rs ≤700 行（最大 tests.rs 581 行）。
- 未覆盖清单（切片外，遇之以 OutOfSlice 响亮拒绝，不静默近似）：
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
