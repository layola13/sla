# 分歧登记 — 子系统线 B（sla-parser）

参照源：Zig `src/parser.zig`（4,478 行），本机只读仓
`~/workspace/local-sa/sa_plugin_sla/`。AST/Token 契约消费自 sla-ast /
sla-lexer 两 crate（path 依赖，协调线接线形态），本线未自带镜像类型。
分类：(a) 文法接受面与 Zig 不一致 · (b) Rust 纪律逼出的重构点 ·
(c) Zig parser 源码可疑底层问题。
原则：分歧先登记再分类，不许为对拍绿静默迁就。

## Parser

### D-P1 (a) 切片未覆盖声明：trait / impl / @overload —— 响亮拒绝（本线缺陷，非 Zig 分歧源）
- Zig：`parseDecl` / `parseAtDecl` 完整接受三者。
- Rust 现状：三者一律 `SyntaxError`（expected 注明未覆盖），不做半镜像。
  最小输入：`trait T { fn m(); }`、`impl Foo { }`、`@overload fn f();`
- 处置：列入未覆盖清单候后续线补全；补全前不得以近似形态冒充覆盖。

### D-P2 (a) 模板字面量未覆盖 —— 响亮拒绝
- Zig：`parsePrefixExpr` 对 `template_start` 做 format 调用解糖（与
  lowering 的模板展开面联动）。
- Rust 现状：`TemplateStart` 前缀一律 `SyntaxError`（未覆盖）。
  最小输入：`` fn f() { let s = `x${1}`; } ``
- 关联：块跨度计算（D-P7）在含模板字面量的良构输入上与 Zig 一致性
  未对拍，随模板覆盖一并复核。

### D-P3 (a) `.sla` import 预扫描缺位 —— 类型名识别依赖外部 seed
- Zig 位置：`parser.zig:1294-1319` `parseImportDeclAfterAt`：`.sla`
  路径除记录模块名外还同步预扫描被导入文件（`prescanSlaImportTypes`），
  使导入的 struct/enum 名在本文件后续能被识别为 struct 字面量 / 枚举
  字面量 / let-else 模式门。
- Rust 现状：只记录模块名（`record_import_module_name`）；类型名须由
  对拍脚手架经 `Parser::seed_known_type_names`（镜像 Zig 的
  `seedKnownTypeNames` 通道）注入。未 seed 时 `Imported { ... }` 不被
  识别为 struct 字面量，与 Zig 预扫描成功时接受面不一致。
- 处置：有意不把文件系统 IO 塞回解析器（理由见 D-P11）；seed 通道即
  预扫描的显式化。协调线对拍时负责提供 seed。

### D-P4 (b) 标记流预扫描 vs 增量 lexer + 值快照
- Zig：Parser 持有 lexer 增量拉取，试探解析（三元、泛型尾）靠
  `(lex, tok)` 值快照回滚。
- Rust：`sla_lexer::Lexer` 非 `Copy` 且状态私有、无快照接口 →
  `Parser::new` 预扫全部 token 入 `Vec<Token>`，快照退化为 token
  下标。lexer 为确定性纯函数，token 流逐个相同；仅结构不同。

### D-P5 (b) 共享指针 AST → owned 克隆（别名关系消失）
- Zig：多处解糖把同一 `*Node` 指针放进 AST 两处——`+=` 等复合赋值
  的 target 与二元左操作数同体、`??=` 的 target 同理、`?.` 解糖的
  `cond` 与 `let_chain[0].value` 同体、链式赋值的多个 target 各自
  别名同一临时量节点形态。
- Rust：owned 树无法别名，统一 `clone()`。AST 形状逐字段相同，但
  下游（checker/codegen）若依赖指针同一性（如以指针为键的侧表），
  迁移时须改用结构关联——与 D-P6 同源，提请 checker 线注意。

### D-P6 (b) 函数体跨度侧表：指针键 → 解析顺序表
- Zig：`function_body_spans` 以 `*Node` 为键（arena 指针稳定）。
- Rust：`Vec<Node>` 元素会随 Vec 扩容搬家，地址不稳定 → 改为按解析
  顺序记录 `Vec<(usize, usize)>`，信息量相同、关联方式不同。

### D-P7 (b) 块跨度计算：源码重词法 vs token 流计数
- Zig：`peekBlockSpan` / `skipBlockSpan` 在源码字节上重跑一台镜像
  模板状态机的 lexer 数括号（`parser.zig:1517` 起）。
- Rust：在已扫描 token 流上数 `LBrace`/`RBrace` 深度。模板字面块是
  整块 token、插值内括号是真 token，良构输入上结果一致；含模板
  字面量的病理输入未对拍（与 D-P2 一并候复核）。

### D-P8 (b) 错误形态：侧信道 → 错误值自带
- Zig：错误仅 `ParserError` 枚举，期望文案走 `last_expected` 可变侧
  信道，诊断渲染另在 `printDiagnostic`。
- Rust：`ParseError { kind, start, end, expected }` 自带。文案不求与
  Zig 逐字一致（本线口径：先求响亮报错），kind 名与跨度对齐 Zig。

### D-P9 (c) `parseInfixExpr` 的 `.bang` 宏调用分支疑似不可达 ★
- Zig 位置：拦截点 `parser.zig:2779` 一带（`parseExpr` 循环顶部的
  postfix-`!` → `unwrap` 解糖）；被遮蔽分支 `parser.zig:3305`
  （`parseInfixExpr` 的 `.bang` => 宏式调用 `name!(args)`）。
- 理由：`parseExpr` 每轮先于 `getInfixPrecedence` 分发拦截 `!`，
  `parseInfixExpr` 永远拿不到 `bang` 算子。故 `foo!(x)` 实际被解析
  为 `unwrap(foo)` 再对其结果做普通调用（左值非 identifier/closure
  → `InvalidCallTarget`），宏式调用形态在当前 parser 下不可达。
- Rust：为结构对位保留该分支并在代码注释中标注；行为与 Zig 一致
  （同为不可达）。请主线确认宏调用是否走 source_expand 前置展开——
  若是，此分支为死代码候选；若否，`foo!(...)` 是真缺陷。

### D-P10 (c) 三元试探吞掉真实解析错误
- Zig 位置：`parser.zig:3381-3388`（`parseInfixExpr` 的
  `.question_mark` 分支，`catch break :blk null` ×3）；`parseExpr`
  顶部的三元试探同形。
- 理由：then/else 子表达式的任意错误（包括与三元无关的真实语法
  错误）被吞掉后回退，降级为 `try_expr` 或以外层位置二次报错，
  诊断位置与形态均可能失真。差分对拍时此类输入的报错差异应先按
  本条归因，不要当作 Rust 侧缺陷。

### D-P11 (c) import 预扫描：解析器内文件 IO + 失败静默吞没
- Zig 位置：`parser.zig:1313` `prescanSlaImportTypes(...) catch {};`；
  路径解析 `parser.zig:391-417` `resolveSlaStdImportPath` 含环境变量
  与硬编码候选根（含机器专属绝对路径 `/home/vscode/...`）。
- 理由：① 预扫描失败（文件缺失/权限/深度超限）被 `catch {}` 静默
  吞掉，后果是 struct 字面量识别静默退化——属「静默兜底」同族；
  ② 解析结果依赖运行环境与文件系统状态，同一源码在不同机器上可
  能解析出不同 AST 形状（识别门命中与否），差分复现须先固定 seed。

### D-P12 (c) `parsePrefixExpr` 兜底分支直写 stderr
- Zig 位置：`parser.zig:3205`：未知前缀 token 分支用
  `std.debug.print` 直接向 stderr 打印后才返回 `UnexpectedToken`，
  绕开 `printDiagnostic` 诊断通道。库调用者（如宿主进程内嵌解析）
  会收到无格式的 stderr 噪声，且该输出不受任何开关控制。

### D-P13 (c) `ParserError.Overflow` 是死错误种，整数溢出被误归类
- Zig 位置：`parser.zig:21` 声明 `Overflow`、`:253` 备有专门文案
  `"literal within supported range"`，但全文件无一处抛出（grep
  `Overflow` 仅此两命中）；`parsePrefixExpr` 整数分支的
  `parseInt` 失败统一映射为 `InvalidCharacter`。
- 后果：超范围整数（如 `99999999999999999999999`）报「非法字符」
  而非「字面量超范围」，诊断误导。Rust 侧暂随 Zig 归
  `InvalidCharacter` 保持对位，候主线裁决是否改抛 `Overflow`
  （改则两边同改并在此登记转 (a) 已裁分歧）。
