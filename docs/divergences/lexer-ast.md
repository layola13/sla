# 分歧登记 — 子系统线 A（sla-lexer / sla-ast）

参照源：Zig `src/lexer.zig`（524 行）、`src/ast.zig`（483 行），本机只读仓
`~/workspace/local-sa/sa_plugin_sla/`。
分类：(a) 与 Zig 行为/产出分歧 · (b) Rust 纪律逼出的表达受阻点 · (c) Zig 源码可疑底层问题。
原则：分歧先登记再分类，不许为迁就 Zig 静默改形。

## Lexer

### D-L1 (b) 注释重启：递归 → 循环（行为等价）
- Zig：`lexer.zig` `/` 分支行注释后 `return self.next();` 递归重启扫描。
  连续行注释逐层压栈，病理输入（大量连续注释）下 Zig 栈深随注释数增长。
- Rust：`next()` 内层循环 `continue` 重启。Token 产出逐个相同，仅结构不同。
- 处置：保持循环实现；登记备查，不需裁决。

### D-L2 (c) 字符串/模板转义在 EOF 处越界一字节 ★ Zig 侧缺陷候选
- Zig 位置：
  - `lexer.zig` 字符串分支：`while (...) { if (buffer[index] == '\\') index += 1; }` 的
    循环递增使未终结字符串以 `\` 结尾时 `index` 走到 `buffer.len + 1`，
    产出的 `invalid` token 带 `loc.end > buffer.len`。
  - `lexer.zig` `nextTemplate` 字面块：`if (c == '\\') index += 2;` 同样可越界一字节。
- 后果：任何按 `buffer[loc.start..loc.end]` 切片消费 loc 的下游都会拿到越界区间
  （Zig 安全模式 panic、ReleaseFast 下为未定义切片）。
- Rust：两处均钳制到 `buffer.len`；token 的 tag（invalid）与 start 不变，
  仅该 invalid token 的 `loc.end` 与 Zig 相差至多 1。**这是有意不迁就的分歧。**
- 处置建议：主会话转主线缺陷登记（lexer 输入校验类）；Rust 侧保持钳制。

### D-L3 (a) 数字后缀吞食不对称（行为保留，防误「修复」）
- Zig 位置：`lexer.zig` 数字分支。
  - 整数：尾部字母数字后缀被整体吞入同一 token（`32i32`、`0xFFu32` 均为单个
    int_literal；十六进制路径在数字后还依次吞 `[A-Za-z]*`、`[0-9]*`，
    故 `0x1G2` 也是单个 int_literal）。
  - 浮点：一旦判定为 float，后缀**不**吞（`1.5f32` → float_literal + identifier `f32`）。
- Rust：逐字镜像，并有单元测试 `suffix_asymmetry` 钉住。
- 处置：parser 线不得单方面「修正」；如需变更须交协调线裁决并同步 Zig 侧。

### D-L4 (a) 非 ASCII 字节逐字节 invalid（行为保留）
- Zig 位置：`lexer.zig` 标识符/字符分支全部走 `std.ascii.*` 字节判定。
  字符串与注释之外出现的任一 UTF-8 多字节字符，会按**每字节一个** `invalid`
  token 产出（一汉字 = 3 个 invalid）；`skipWhitespace` 只认空格/Tab/LF/CR 四种。
- Rust：逐字镜像。fixtures 中的中文仅在注释/字符串内，不受影响。
- 处置：parser 线对错误恢复的设计须按「invalid 流」而非单点错误预期。

### D-L5 (c) 模板嵌套硬上限 8，无专用诊断
- Zig 位置：`lexer.zig` `tmpl_stack: [8]u16`；第 9 层模板起始 backtick 直接产
  `invalid`，与词法错误共用同一 tag，下游无从区分「嵌套过深」与「非法字符」。
- Rust：逐字镜像（含上限与 invalid 行为）。
- 处置：登记为 Zig 侧诊断面缺口候选；是否增设诊断交协调线。

### D-L6 (c) `interp_depth: u16` 无溢出防护（理论项）
- Zig 位置：`lexer.zig` `{` 分支 `interp_depth += 1` 无饱和/检查。
  插值内 65536 个未闭合 `{` 在 Zig 安全模式 panic、发布模式回绕；
  回绕后 `}` 的收尾判定错乱。
- Rust：`u16` 自增在 debug 下同样 panic、release 下同样回绕——忠实镜像，未钳制。
- 处置：理论风险登记；两实现行为同构，不需裁决。

## AST

### D-A1 (b) 所有权形态：arena 指针网 → owned 树 ★ 结构性受阻点
- Zig：`ast.zig` 全图是 arena 分配的指针网——`*Node`、`[]const *Node`、
  `[]const u8` 借用切片；节点可被多处别名引用，后期 pass 可原地改写已挂入树的节点。
- Rust：owned 树——`*Node` → `Box<Node>`、`[]const *Node` → `Vec<Node>`、
  `[]const u8` → `String`、`*Type` → `Box<Type>`、`?T` → `Option<T>`。
- 受阻点（纪律起作用处）：Zig 里「同一 `*Node` 挂两个父节点」「插入后原地补丁」
  这类写法在 Rust 侧无直接对应；checker/lowering/sab 线如遇这类 Zig 原写法，
  必须改用索引（arena+index）或重建节点，并把该 Zig 位置登记到各自分歧文件——
  这正是本实验要照出的别名/原地改写点位。
- 处置建议：请协调线裁决统一口径（默认 owned 树；确需共享时全仓统一引入
  arena/index 方案，禁止各线私自用 `Rc` 绕行）。

### D-A2 (b) 变体命名映射（机械规则）
- Zig 的 union 变体为 snake_case（`let_stmt`、`func_decl`…），Rust 变体用
  CamelCase 承载同名 payload 结构（`LetStmt(LetStmt)`）；类型名、字段名与 Zig 逐字一致；
  变体声明顺序与 Zig union 相同，便于逐行对读。`Literal`/`Type`/`Primitive` 等
  union/enum 同此规则（如 `int_val` → `IntVal`、`void_type` → `VoidType`）。

### D-A3 (a) Zig 字段默认值在 Rust 构造点显式化
- Zig 多处字段带默认值：`StructDecl.derives = &.{}`、`is_union/is_opaque = false`、
  `EnumVariant.discriminant = null`、`FuncDecl.abi/operator = null`、
  `CallExpr.callee = null`、`MatchCase.guard = null`、`FnPtrType.abi = null` 等。
- Rust：全部字段显式赋值，**刻意不**为这些结构伪造 `Default` impl——伪造默认值
  可能与 Zig parser 实际填充值静默分歧。各结构 doc 注释逐个标注了「Zig default」。
- 处置：parser 线构造节点时须逐字段对照 ast.zig 默认值填入；如需 Default 便利，
  交协调线统一决定。

## 覆盖与验证

- sla-lexer：`lexer.zig` 公共面全量镜像（Token/Loc/Tag 全 90 个变体、Lexer 全部
  状态机含模板字面量、checkKeyword 全 33 个关键词）。Zig 自带 2 个 test 已移植，
  另加模板与后缀不对称 2 个测试及 fixtures 全量词法扫（无 invalid、终局 eof）。
- sla-ast：`ast.zig` 全量镜像（Node 全 51 个变体及全部 payload 结构、完整 Type 系统），
  附 fixture 04 形状的构造测试。AST 为纯数据类型，无 Zig 测试可移植（原文件无 test 块）。
- 构建：Kaggle 场地（2201，cargo/rustc 1.99.1）`cargo test` 两 crate 全绿（6 测试），
  强制重编零警告；日志 `docs/build-logs/lexer-ast-build-20261107.log`。
- 未覆盖清单：无（限于本线辖区的两文件）。parser/checker/lowering/sab 对本 crate 的
  消费面在各分线，本线未预判其便利 API（如 Default、Display），候协调线裁决后补。
