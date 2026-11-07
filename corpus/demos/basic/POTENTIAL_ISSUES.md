# SLA 潜在问题追记（basic demos 反哺）

在参考 `tsgosa/demos` 丰富 `demos/basic` 时，用 `sa sla check/test/build-exe` 实际验证，
发现以下候选问题。状态分为：已确认 / 待复核 / 已规避（demo 侧绕行）。

## #1 定长数组元素类型错配：check 放行，运行值错（已修复：check 拦截）

- 复现：`fn array_sum(values: [i32; 5])` + 调用 `array_sum([3,1,4,1,5])`，
  字面量按 `int`(i64, 8B stride) 分配 40B，形参按 `i32`(4B stride) 读取，
  `check` 通过，`test` 在求和断言上 panic（`demos/basic/08_arrays` 初版 panic 804）。
  SA 后端（`--test-backend sa`）复现 panic 1001，SAB 后端因 `expected_ty` 路径碰巧通过。
- 期望：`check` 应对 `[i64;5]` → `[i32;5]` 报类型错，或自动统一字面量元素类型。
- 已修复：`src/type_checker.zig` 在全部 `plainCallArgMatches` 调用点后加
  `arrayCallArgStrictMatches` 宽度检查，错配报
  `array element type mismatch (stride differs)`；`/tmp/probe_arr.sla` 现 `check` 退出 1。
- 现状规避：`08_arrays` 改用 `[int; 5]`（与 `tests/test_unit_arrays.sla` 一致）。
- 归属：前端类型检查（Phase 5 聚合布局相关）。

## #2 `if` 表达式作返回值需要 `return if ...`（写法约束，非 bug）

- `fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }` 可通过，
  但多行函数体中 `return if c > 10 { 100 } else { 200 };` 更稳。
- 已在 `03_if_else / 06_ternary` 两种写法各保留一种，验证均通过。

## #3 字符串字面量类型推断：无注解绑定 str_eq 失败（已修复）

- 复现（/tmp/probe_str.sla）：`str_eq("hello","hello")` 通过；
  `let s = "hello"; str_eq(s, "hello")` panic；
  `let s: ptr = "hello"; str_eq(s, "hello")` 通过。
- 根因：无注解绑定运行时是 Slice，但检查器暴露为 raw_ptr；
  SAB 的 `str_eq` 指针相等快径与 `CSTR_FROM_PTR` 把 Slice 结构体误当 C 字符串。
- 已修复：SAB 以 `inferred_string_slice_locals` 识别无注解绑定，
  快径排除 + 参数直通 Slice；`/tmp/p3_unannot.sla` 与 `/tmp/p3_main.sla`
 （`EQ`）在默认/SA 双后端全绿。
- 现状（已消除绕行）：`10_strings` 已合并为单绑定，另保留 `: ptr` 对照断言。
- 关联：`current_plan.md` 提到 `str_eq` 相关 known issues；`68_parser_tokens`（数组/参数来源的 ptr）不受影响。

## #4 模板字符串多插值：check 与 test 前端不一致 + 特定组合 runtime 崩溃（已修复 check 侧；残留 #4b）

- `check` 拒绝多插值模板：`println(`hi ${name}, n=${n}!`)` 报 `ExpectedDeclaration`，
  但同文件 `sa sla test`（SAB 路径）可通过（`tests/test_unit_template_string.sla` 亦如此：check 失败、test 3/3 通过）。
- 根因（check 侧）：`check` 以 `.parse_test_bodies = false` 解析，`@test` 体走
  `skipBlockSpan/peekBlockSpan` 跳块；两者按裸花括号计数，模板 `${...}` 的插值
  `}` 被误作块结束，块被提前截断，剩余 `, n=...` 在顶层报 `ExpectedDeclaration`
 （单插值在 `@test` 内同样触发；`fn` 体因正常解析不受影响）。
- 已修复：`src/parser.zig` 两处跳块扫描镜像词法模板状态机
 （`template_start/interp/end` + `interp_depth` 栈），`/tmp/p4_multi.sla`、
  `/tmp/p4_single_test.sla`、`ptr+多插值` 的 `check` 均通过，`test` 双绿；
  本轮复测 `ptr + 多插值` 的 `unreachable` 崩溃已不再出现
  （`: ptr` 绑定持有真 C 串后 CSTR 路径正常）。
- 现状规避：`10_strings` 的打印统一用 `"..."` 格式化（见其 README）。
- 残留 #4b（已修复，2026-10-10）：反引号模板脱糖为嵌套 `format()` 调用
  （`println(format(...))`），此前双后端 `println` 仅当首参为字符串字面量才
  展开占位，嵌套调用只输出换行。现单参 string-like/format-string 走既有
  per-arg 发射（SAB 经 `planPrintlnArg` + `emitPrintlnArg`，SA 镜像同发射），
  其余元数保持裸换行历史行为。回归 `94_nested_tpl` 已补直打断言。
  模板断言仍可用 `str_eq(format(...), ...)` 或 `"..."` 直调。

## #5 `println("{}", s)` 与 `str_eq(s, …)` 对字符串变量注解的要求互斥（已修复）

- 复现：`str_eq` 要求显式 `: ptr`（#3），但 `println("{}", s)` 对 `: ptr` 绑定
  在 SAB 原生 `build-exe` 运行时崩溃（`signal 6 / reached unreachable code`，
  /tmp/probe10e.sla；本轮复现为 `/tmp/p5_noimp.sla` build-exe `abort 134`，
  而 `test` 双后端通过、`sa run` 正常）。
- 根因：SAB 的 `: ptr` 绑定借用 dance 引用了未定义的寄存器
  （反汇编见 `borrow r17,r18` 且 `r18` 无定义），`println` 再把数据指针
  当 Slice 解引用（`load +0/+8`）。
- 已修复：SAB 的 `: ptr` 字符串绑定直接持有 NUL 结尾数据指针
  （`explicit_ptr_string_locals`），`println/len/format-push/as_ptr` 对其走
  C 字符串路径；`/tmp/p5_noimp.sla` build-exe 输出 `hello`，退出 0。
- 现状（已消除绕行）：`10_strings` 已合并为单绑定，另保留 `: ptr` 对照断言。
- 归属：`println`/格式化路径与字符串变量物化（SAB emitter）。

## #6 复合赋值仅支持 `+= |= &=`，`-= *= /= %=` 无词法 token（已修复）

- 复现（/tmp/probe_op.sla）：`x += 1` 通过；`x -= 1` / `x *= 2` / `x /= 4` / `x %= 5`
  均报 `Unexpected prefix token: equal`；对照 `x |= 1` / `x &= 1` 通过。
- 根因：`src/lexer.zig:62-106` 的 token 枚举仅定义 `plus_equal`、`pipe_equal`、
  `ampersand_equal`（另有 `question_question_equal`），缺失 `minus_equal` /
  `asterisk_equal` / `slash_equal` / `percent_equal`，故 `-=` 等被切分为 `-` + `=`。
- 期望：在 lexer + parser 补齐四种复合赋值（或 `check` 给出明确不支持提示）。
- 已修复：`src/lexer.zig` 新增四种 token 及 `-/ * / / / %` 后接 `=` 分支；
  `src/parser.zig` 在 `parseStmt` 脱糖为 `sub/mul/div/mod` + `assign`，
  并补入 `genericLookaheadBoundary`；`31_compound` 已改回原生写法，全量 50/50 全绿。

## #7 全 return 臂的 switch 函数：check 与 SAB 后端要求互斥（已修复）

- 初版（各臂均 `return`，尾表达式即 `switch`）：`check` 与 `test` 均报
  `TypeMismatch in function tail expression: expected i32, actual void_type`，
  因 `switch` 本身是 `void` 语句。
- 加尾兜底 `return 3;` 后：`check` 通过，但 SAB 后端（`test`）报
  `FallthroughForbidden: basic blocks must end with jmp, br, br_null, or return`
 （不可达兜底块缺少终止子，SAB emitter 缺口）。
- 根因：三处不识别全发散 `switch` 尾——检查器 `exprTerminates` 无 `switch` 分支；
  共享 `lowering_rules.exprTerminates` 同缺；SAB 函数尾声无条件发射取值尾声；
  SAB/SA 的 `switch` 合并哨兵无条件发射（不可达指令落到终止子之后）。
- 已修复：检查器 + 共享层 `exprTerminates` 新增 `switch` 臂
  （全臂发散 + 含 `default` 才算终止）；SAB 函数尾声对终止体跳过取值尾声；
  SAB/SA 的 `switch` 合并块仅在可达时发射（有 fallthrough、无 default、
  或末臂非 default）。探针 `/tmp/p7a.sla`（纯 `return` 臂尾 `switch`）
  `check + test`（SAB/SA 双后端）全绿，build-exe 运行返回 20 正确。
- 现状：`33_switch_return` 保留已验证的混合写法（仍双绿）；
  纯 `case: return` 尾 `switch` 风格现已同等支持。

## #8 默认参数值不支持：`fn f(a: i32 = 1)` 直接 parse 失败（已决策：明确拒绝）

- 复现（/tmp/p_def2.sla）：`fn f(a: i32 = 1, b: i32 = 2) -> i32` 报
  `Syntax Error ... error.SyntaxError`（形参位置）。
- 决策：语言规范 §15 明确列为不支持；解析器对形参 `=` 给出针对性提示
  （`expected r_paren (default parameter values ... see demos/basic/47_default_args)`，
  探针 `/tmp/p8.sla` 已验证）。
- 官方绕行：`47_default_args` 用 `f_default()`（零参）+ `f_full(a,b,c)`（全参）
  表达 `f()=6, f(10,2,3)=15, f(10,20,30)=60` 语义（README 与规范口径一致）。

## #9 Label（`outer:` / `continue outer`）不支持（已决策：明确拒绝）

- 复现（/tmp/p_lbl2.sla）：`outer: for i in 0..3` 报 `Syntax Error`。
  无 label 的内层 `break/continue` 正常（/tmp/p_lbl.sla check 通过）。
  `break/continue` 后不接受标签名（`continue outer` 报 `expected semicolon`）。
- 决策：语言规范 §15 明确列为不支持；解析器对语句首 `name:` 给出针对性提示
  （`expected semicolon (loop labels ... see demos/basic/48_nested_break)`，
  探针 `/tmp/p9.sla` 已验证）。
- 官方绕行：`48_nested_break` 用内层 `break` 改写 `continue outer`
  （本用例等价，`t=3`；一般的跨层 continue 仍无对等写法，需显式重写；
  README 与规范口径一致）。

## #10 `len()` 作用于字符串返回表示字数而非字符数（已修复）

- 复现（/tmp/p_strlen*.sla）：`len("hi there")` / `len("")` / `len("hello")`
  曾返回 `2`（疑为胖指针表示字数）；本轮复测字面量 `len` 已返回正确字节数
  （`5/0/8`，`test` + build-exe 双绿）。
- 根因（变量情形）：SAB 的 `len()` 对 raw_ptr 统一做 `CSTR_LEN` NUL 扫描，
  而无注解变量实际是 Slice，扫描结构体字节得垃圾值
 （本轮复现：`/tmp/plen_var.sla` 默认后端 `panic 2001`，SA 后端通过）。
- 已修复：SAB 的 `len()` 对 `inferred_string_slice_locals` 改读 Slice 长度字段；
  `/tmp/plen_var.sla` 双后端全绿。
- 现状：字符串相等断言统一用 `str_eq`（见 #3），`len(str)` 字面量与变量均已验证。

## #11 数字分隔符字面量（已修复，2026-10-10）

- 原复现（/tmp/psep.sla）：`let x = 1_000;` 报
  `found '_000', expected semicolon`（词法数字段遇到 `_` 即停，后续当标识符）。
- 修复：词法仅保留数字间 `_`（十进制/浮点小数两侧/十六进制；尾随与孤立 `_`
  保持历史切分行为）；parser 五处转换点（整数/浮点/后缀/重复数组长/定长类型长/
  枚举判别式）经 `stripNumericSeparators` 去分隔符后沿用原逻辑。
- 验证：`1_000+2_000`、`0xFF_FF`、`7_000.5`、`1_000i32`、`0..1_0` 双后端全绿 +
  build-exe 输出核对；`1_`、`1__000` 仍为同类语法错误；回归 `55_num_sep`
  已改直写分隔符形式。

## #12 链式赋值（已修复，2026-10-10）

- 原复现（/tmp/pchain.sla）：`a = b = 5;` 报 `found '=', expected semicolon`
  （赋值语句不返回值，属明确拒绝）。
- 修复：parser 语句层将 `=` 右结合链 desugar 为块
  `{ let __chainK = value; tn = __chainK; …; t1 = __chainK; }`
 （`__qcN` precedent；右向左赋值；纯 `=` 链，`let x = a = 5` 与混合 op-assign
  不在范围）；单赋值路径逐字保留。单态化/checker/双后端零改动（块/let/赋值
  均为既有节点）。
- 验证：双/三链（含函数调用右值）与字段/索引目标双后端全绿 + build-exe
  输出核对；回归 `56_chain_ops` 已改直写链式。

## #13 闭包字面量直接调用（已修复，2026-10-10）

- 原复现（/tmp/piife.sla）：`(|x: int| x * 2)(21)` 报
  `found '21', expected callable function ... (error.InvalidCallTarget)`。
- 修复：`ast.CallExpr` 加 `callee: ?*Node`（默认 null，零兼容影响）；
  parser `l_paren` 位仅放行 `.closure_literal`（其余保持原拒绝文案）；
  checker 首分支镜像命名闭包路径做元数/类型检查；
  SAB/SA 发射首分支直达既有 `genClosureCall`；
  单态化 5 处重建点透传并特化 `callee`（本轮真凶：重建丢字段导致空名调用）；
  18 处 `exprNeeds*` + assigned/addressable/future-runtime/preload/escaped/thread
  visitor 同步覆盖 callee 子树。
- 验证：位置矩阵（let/println 实参/嵌套调用/嵌套闭包/while cond/if cond）
  双后端全绿 + build-exe 输出核对 + 三拒绝路径（非闭包 callee 沿用
  InvalidCallTarget；元数/类型错配各有明确错误）；回归 `57_iife` 已补直接调用断言。
- 现状：`57_iife` 先绑定写法保留为等价写法，直接调用为推荐写法。

## #14 HashMap 在 SAB 默认后端报 UnknownRegister（已修复，2026-10-10）

- 原复现：两次 `insert` + `get().copied().unwrap_or_default()` 在 SAB 默认后端报
  `trap UnknownRegister 1007 ... "callee is not declared"`；`--test-backend sa` 通过；
  仓库内 `demos/rosetta/53_cache_hits` 同症。另注意 `sa sla test` 对零 `@test`
  文件不执行 `main`（m1–m6 空转教训），回归断言必须进 `@test`。
- 根因：SAB 对 `HashMap.get` / `Option.copied` / `Option.unwrap_or_default`
  全无 lowering，通用路径放出 `@sla__get` 等不存在的 callee。
- 修复：SAB 加三处 direct lowering 并挂入 `genCall` 分发（`genMapInsertCall` 之后）：
  `genMapGetCall`（`MAP_TRY_GET` + `OPTION_NEW_SOME/NONE` 折叠，miss 得 `None`，
  与 SA 的 `SLA_MAP_TRY_GET_OPTION` 同语义）；`genOptionCopiedCall`
  （`OPTION_COPIED_U64`）；`genOptionUnwrapOrDefaultCall`
  （`OPTION_UNWRAP_OR_DEFAULT`，零值默认与 SA 一致）。接管条件均为 receiver
  类型匹配（HashMap/Option），其余保持原 fallback。
- 后续（2026-10-10）：`BTreeMap.get`（`genBTreeMapGetCall`，值直存经栈槽中转，
  与 SA 的 `SLA_BTREE_MAP_TRY_GET_OPTION` 同语义）与 `len()` 的 BTree 分支
  （`genVecLenCall` 内 `BTREE_MAP_LEN`）一并补齐；空表缺失/覆盖写/多键/缺键/
  表长矩阵双后端全绿 + build-exe 输出 `10,2,0,2` 核对。
- 后续（2026-10-10）：`map.clear()` 双容器支持（checker `isClearCall` 分支定
  void 类型；SAB 经 `MAP_CLEAR` / `BTREE_MAP_CLEAR` 宏，SA 直调
  `@sa_map_clear` / `@sa_btree_map_clear`；清零后重插矩阵双后端全绿 +
  build-exe 输出 `0,3,0,8` 核对）。`167_map_clear` 可据此落地。
- 验证：空表缺失/覆盖写/多键/缺键矩阵双后端全绿 + build-exe 输出 `10,2,0` 核对；
  `53_cache_hits` 通过。

## #15 `String` 形参端到端不可用（已确认缺口，本轮探针结论）

- 现象：`fn echo(s: String) -> String { return s; }` + `echo("hi")`：
  `check` 通过，SAB `str_eq` 恒假（`@sla__echo(^s: ptr)` 借入栈 Slice，
  返回借用指针再与字面量做指针相等）；SA 后端报 `CapabilityMismatch`。
  小写 `s: string` 注解亦然（`isStringType` 只认 `String`，属同一缺口；
  曾尝试 parser 层小写映射，但单映射无一可用用例——`let s: string = "hi"`
  报 declared/val 标签错配——且把 loud 的 check 错误推迟为运行期静默错，
  故已回退，映射须与值格（字面量/Slice→String 强制）同批交付）。
- 连 `let t = "hi"; echo(t)` 亦报 arg TypeMismatch（Slice vs String），
  故暂无绕行；字符串请用推断绑定 + `str_eq`/模板（见 10/68/69/94）。
- 范围：字面量/绑定实参→String 形参的借入 ABI（`^s: ptr`）、返回值保持、
  `str_eq` 对借入 Slice 的处理、SA 侧同形调用实参物化。
- 根因（2026-10-10 反汇编钉死）：`@sla__echo(^s: ptr)` 体的 `return_` 直接返回
  借用指针；调用方重建 Slice 时按 Vec 布局取 len@16，而借入实为栈 Slice
  （len@8，应按 SliceAbi 取 len@8），长度读成垃圾致 `str_eq` 恒假。
  即 String（Vec-backed，len@16）与 Slice（len@8）两套表示在借入链路上
  混用。修须统一 String 形参约定（借入 Slice 全链按 SliceAbi 解引用，
  或调用方转 owned-String 后全链按 Vec 布局），属值格特性，不半修。
- 探针：`/tmp/probe_strarg.sla`、`/tmp/probe_sgen*.sla`、`/tmp/probe_capstr.sla`、
  `/tmp/probe_letmid.sla`、`/tmp/probe_letann.sla`、`/tmp/probe_sbody.sla`。

## #16 `BTreeSet<int>` 静默错键（已修复，2026-10-10）

- 现象：`BTreeSet::new()` + `s.insert(1)` 双后端 `test` 全绿（`check` 因跳过
  `@test` 体而空转，见 #4），但 `contains(1)` 恒假、`len` 亦错——整数被当
  指针存槽，永不命中。在仓 `codegen.zig` 的 `set code generation` 单测亦用
  int 键（把 bug 写进断言），一并改为字符串键。
- 修复：checker `btreeSet insert/contains` 加 `btreeSetKeySupported` 门
  （仅 raw_ptr 字面量/Slice；整数等标量报 TypeMismatch loud 错误）。
  `BTreeSet<string>` 在仓用例不受影响。
- 验证：int 键 `test` 报 TypeMismatch；string 键双后端绿；正反单测各一
  （`type checker rejects scalar btreeset keys`）。

## #17 函数返回定长数组值语义错（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`fn get() -> [i32; 2] { return [1, 2]; }` 可过 `check`，但 `test`
  读回值错——直接索引 `get()[0] + get()[1]` 与绑定后 `a[0] + a[1]` 均
  断言失败（探针 `panic=9007/9008/9004`，4/4 确定性复现，非偶发）。
- 根因（待编译器专轮定位）：数组按值返回 ABI；与 #15 同属值格特性，不半修。
- 现状规避：106–115  batch 中 110 只做局部数组存取；需跨函数传递数组时
  改用 `Vec`（如 112）或模块级数组；JEV 单选（record 84%）裁定记档绕行。
- 复现探针：`/tmp/probes/p_arrret.sla`、`/tmp/probes/p_arrret2.sla`。

## #18 字符串关系比较绑定左值失真（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：推断绑定作 `<`/`>` 左值时失真——`let a = "ab"; a < "abc"`
  在 exe 中恒假；字面量直接写进 `@test` 体求值则 SAB 报
  `MemoryLeak` trap（live Composite register，函数出口未释放）；
  字面量左值（` "ab" < b`）与字面量间比较正常。
- 定性：与 #3/#5/#10 同属字符串物化家族；`fn f(a: string)` 形参注解
  本就不支持（#15，TypeMismatch）；2026-10-10 追测：连 `: ptr` 注解绑定
  作左值在 exe 中亦错（`u_annstr` 输出 `0`，期望 `1`），故缺口不限于
  推断绑定——凡绑定左值皆失真，仅字面量可比。
- 现状规避：116–125 batch 中 122 只用字面量比较，且包进无参 helper fn
  再由 `@test` 断言（探针 `r_strcmp6` 双后端 + exe `-1` 核对）；
  需比较变量字符串时改用 `str_eq` 判等；2026-10-10 缩小：`switch` 分发
  不受影响——`: ptr` 绑定 scrutinee + 字面量臂三线全绿（180），
  失真仅限 `<`/`>` 关系比较。
- 复现探针：`/tmp/probes2/r_strcmp.sla`（绑定左值 exe 失真）、
  `/tmp/probes2/r_strcmp4.sla`（矩阵 `9,9,-1,9`）、
  `/tmp/probes2/r_strcmp5.sla`（`@test` 直写触发 MemoryLeak）。

## #19 顶层 `const` 结构体双后端失联（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`const O: P = P { x: 1, y: 2 };` 可过 `check`，但 `test` 与
  build-exe 均报 `UnknownRegister` trap（结构体常量对后端不可见）；
  顶层 `const` 标量/f64/数组正常（133/136 双后端全绿），顶层 `let`
  直接 parse 拒绝（ExpectedDeclaration）。
- 现状规避：136–145 batch 中 136 只用顶层常量数组，144 用局部结构体；
  需全局共享结构体时暂以函数返回构造表达（见 111）。
- 追测（2026-10-10）：顶层常量数组作值参亦有问题——`first(A)` 可过 check、
  exe 正确（`1`），但 `@test` 内报 MemoryLeak trap；局部数组作值参正常
  （116/127/140）。故顶层 `const` 复合体（结构体/数组传值）一律以后端
  失联/泄漏对待，只做全局只读。
- 复现探针：`/tmp/probes2/t_topstruct.sla`、`/tmp/probes2/y_topparam.sla`。

## #20 嵌套定长数组读崩溃（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`const N: [[i32; 2]; 2]` 可过 `check`；`len(N)` 与 `len(N[0])`
  正常，但 `N[1][0]` 在 `test` 后端触发 signal 11，在 build-exe 中打印
  前缀后 SIGSEGV（确定性复现，双路径一致）。
- 现状规避：多维数据一律用扁平行主序 + 手工索引（见 19/28/58），
  不用嵌套定长数组。
- 复现探针：`/tmp/probes2/t_nestarr.sla`。

## #21 `&&`/`||` 为急求值（已确认语义分歧，2026-10-10，记档不断言修复）

- 现象：`false && eff()` 与 `true || eff()` 仍执行右侧（build-exe 打印
  两次 EFF；`@test` 结果值与短路一致故不断言区分），与 TS 短路语义不同。
- 定性：急求值本身自洽（纯函数无观察差异），仅含副作用时可观察；
  138 以 exe 输出锁定当前语义，后续若实现短路需同步更新 138。
- 复现探针：`/tmp/probes2/t_short.sla`。

## #22 `switch true` 条件臂 exe 失联（已确认缺口，2026-10-10，记档绕行）

- 现象：`switch true` + 布尔表达式臂（`x > 10 =>`，括号与否皆然）
  可过 `check` + `test`（trivial 体），但 build-exe 报
  `MissingType` lowering 错误；布尔字面量臂（`true =>`）三线全绿。
- 定性：与 #19/#20 同属“前端放行、后端失联”类，但为 loud 错误
  （非 trap/崩溃），严重度低；按既有裁定先例直接记档，不另问 JEV。
- 现状规避：166 只用布尔字面量臂；条件分发改用 `if/else if` 链（115）。
- 复现探针：`/tmp/probes2/w_swtrue.sla`、`/tmp/probes2/w_swtrue2.sla`。

## #23 Option 字段解包在 `@test` 内泄漏（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：含 `Option` 字段的结构体可过 `check`；exe 全对（`unwrap` 得 5、
  `match` 得 true），但 `@test` 内任何载荷提取（`unwrap`/`match`、
  单次/多次、helper 包裹皆然）均报 `MemoryLeak` trap；结构体本身构造
  与 `.a` 字段读在 `@test` 内正常；复用同一绑定二次提取另报
  `UseAfterMove`（值语义搬移）。
- 定性：与 #15/#17 同属值格特性，不半修（JEV 单选 record 12%，分歧大但
  批量优先；#19 同轮亦 12%，同类缺口裁定口径一致）。
- 现状规避：本轮不落地 Option-字段 demo；需可选字段时以哨兵值 +
  `bool` 标记表达（见 144 的 bool 字段）。
- 复现探针：`/tmp/probes2/x_optstruct.sla`（泄漏）、
  `/tmp/probes2/x_optstruct2.sla`（复用搬移）、
  `/tmp/probes2/x_optstruct3.sla`（单次 helper 仍漏）、
  `/tmp/probes2/x_optstruct4.sla`（match 提取仍漏）。

## #24 `Vec<f64>` 存读按位错乱（已确认缺口，2026-10-10，记档延修）

- 现象：`Vec<f64>` 可过 `check`；`len()` 正确，但元素读回恒错——
  `push(1.5)` 后 `v[0]` 在双后端皆得 `0.0`（`5e-324` 量级，确定性复现）；
  定长 `[f64; N]` 数组存读正常（110 三线全绿）。
- 根因（已钉死）：`@sa_vec_push(^vec, value: u64, …)` 的值形参为 `u64`，
  f64 在 SA 调用边界被数值转换而非按位存；且双后端症状分歧——
  SAB 侧 index 读回 `0.0`，exe 侧 `pop` 回 `4.6e18`（位模式的数值重解释），
  证明 push/index/pop 三处转换各异，非单点 stride bug。
- JEV 单选曾裁 fix_now（84%），但根因探明后复议：双后端多站点按位
  修复超出本批 scope， partial fix 必致后端分歧（更坏）；故记档延修，
  本分析即未来专轮的修复输入（修复点：push/index/pop 三处 f64/f32
  按位传输，SAB + SA-text 双后端）。
- 现状规避：浮点集合一律用定长数组（110/194）；`Vec` 只装整数/布尔。
- 复现探针：`/tmp/probes2/y_vecf64.sla`（读零）、
  `/tmp/probes2/y_vecf64b.sla`（index 写读正常，对照）、
  `/tmp/probes2/y_vecf64c.sla`（变量 push 同坏）、
  `/tmp/probes2/y_vecf64pop.sla`（pop 回 4.6e18）。

## #25 枚举 `==` 双后端恒假（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：显式判别值枚举（`enum Color { R = 0, G = 1, B = 2 }`）可过 `check`；
  同值 `==` 在双后端皆为假（`G == G` 进 `else`，复现探针 `probe_enum_eq2`
  以 `panic(9003)` 双后端确定性复现）；`!=` 相应恒真。
  另有两条 parser/checker 前置约束：`Color.G` 点式路径不可解析
  （须 `Color::G`）；`if` 条件内直写 `Color::R` 字面量报
  `found '1', expected identifier`（须先 `let` 绑定再比较）。
- 定性：与 #21/#22 同属“前端放行、语义失真”类；`match` 版枚举分发
  （149）正常，故失真限于 `==`/`!=` 算子 lowering；`as i32` 转枚举
  被 checker 拒识（仅数值源可转），无旁路；JEV 单选 record 62%
  （批量优先）。
- 现状规避：枚举相等一律用 `match` 表达（149 口径）；1363 类用例暂略。
- 复现探针：`probe_enum_eq.sla`（绑定版，check 过、test 9002）、
  `probe_enum_eq2.sla`（9003 双后端复现）、`probe_enum_int.sla`
  （`as i32` 拒识对照）。

## #26 捕获型闭包直接调用 check/test 不一致（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`(|x: int| x + k)(5)`（k 为外层绑定）可过 `check`，
  但 `test` 报 `UndefinedVariable: k`（复现探针 `probe_iife_cap.sla`）；
  非捕获嵌套直接调用（864，`206` 已落地）双后端正常；
  先绑定再调用的捕获语义（20/77/164）正常。
- 定性：与 #4 同属 check/test 前端不一致；JEV 单选 record
  （53/47 分歧大，批量优先）。
- 现状规避：捕获型闭包一律先绑定再调用（20 口径）；861 类用例暂略。
- 复现探针：`probe_iife_cap.sla`。

## #27 `Vec.push` 作值 check/test 不一致（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`let n = v.push(2)` 可过 `check`，但 `test` 报
  `binary_expr TypeMismatch`（复现探针 `probe_push_ret.sla`，`n != 2` 处）；
  `push` 作语句（141 口径）双后端正常；多参 `push(2, 3)` 被 checker
  以 `InvalidArgsCount` 拒识（定元数）。
- 定性：与 #4/#26 同属 check/test 前端不一致；JEV 单选 record 74%。
- 现状规避：`push` 只作语句；需表长时用 `len(v)`（584 类用例暂略）。
- 复现探针：`probe_push_multi.sla`（定元数拒识）、
  `probe_push_ret.sla`（作值不一致）。

## #28 SA 后端绑定 void 闭包 lowering 失败（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`let f = || hit(); f();`（hit 为 void 函数）可过 `check`，
  SAB 后端 `test` 绿，但 SA（LLVM）后端报
  `Instruction has a name, but provides a void value!`；
  `println` 作闭包体另报 `ForbiddenSyntax/InvalidMacroInvocation`；
  int 体绑定闭包（57 口径）双后端正常；void 函数直调
  （208/211/213/214）双后端正常；直接调用 void 闭包
  `(|| hit())()` 则连名字决议都不过（`Undefined call: hit`，归入 #26 类）。
- 定性：SA-text 后端 void 值命名缺陷；JEV 单选 record 73%。
- 现状规避：void 效果一律直调 void 函数；670 类用例暂略
  （块体闭包另有解析层拒识：闭包体仅表达式）。
- 复现探针：`probe_void_extra.sla`（SAB 绿/SA 败）、
  `probe_clo_void2.sla`（LLVM void 报错）、`probe_void_iife.sla`
  （直接调用名字决议败）。

## #29 f64 `{}` 打印向零截断（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：f64 值在 `@test` 内精确（`4.5`/`7.5`/`-1.25` 相等断言全过），
  但 `println("{}")` 显示向零截断（`7.5→7.0000000000`、
  `-1.25→-1.0000000000`、`4.5→4.0000000000`），exe 三线复现。
- 定性：显示层数值转换（#24 同类调用边界问题，但限显示不断言值）；
  JEV 单选 record 87%。
- 现状规避：裸 f64 永不直打，一律比较转 int 旗（125/194/216 口径）；
  454 类用例暂略。
- 复现探针：`probe_f64print.sla`、`probe_f64print2.sla`。

## #30 `BTreeMap<int>` 插入三线崩溃（已修复，2026-10-10，JEV 裁定本轮修）

- 现象：单条 int 键 `insert` 即三线 SIGSEGV（test 双后端 signal 11 +
  exe 139）；str 键正常（191）；int 键 HashMap 正常（217 已落地）。
- 根因：与 #16 同胞（BTree 键槽按指针/字符串布局，标量键非法）；
  JEV 单选 fix_now 84%，对齐 #16 修法：checker insert/get 加
  `btreeSetKeySupported` loud 门（`src/type_checker.zig` +2 行注释 +2 门 +
  单测 `type checker rejects scalar btreemap keys`；`zig build test`
  314/314→315/315；191 不回归；探针复测 check 即拒）。
- 现状：BTree 只用 string 键（191/192/222 口径）。
- 复现探针：`probe_intbtree.sla`（双 get 版）、`probe_intbtree2.sla`
  （单插 + len 版）、`probe_intbtree3.sla`（单插最小版）。

## #31 绑定嵌套闭包字面量解析拒识（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`let mk = || |x: int| x * 3;` 报 `InvalidCallTarget` 解析拒识；
  直接嵌套调用已修（#13，206 已落地）；loud 错误非静默坏。
- 定性：#13 的绑定位残留；JEV 单选 record 82%。
- 现状规避：嵌套闭包一律直接调用（206 口径）；386 类用例暂略。
- 复现探针：`probe_clofactory.sla`。
- 追测（2026-10-10）：`match` 判别式仅支持路径模式，int 字面量臂
  （`90 =>`）解析拒识（149/152 均为路径臂）；int 分发用 `switch`
  （33/107 口径）。复现探针：`probe_matchint.sla`。
- 追测（2026-10-10）：`match` 要求枚举 scrutinee，bool 变量作判别式
  解析拒识（166 的 bool 分发用 `switch`，272 的枚举下标可用）。
  复现探针：`probe_matchbool.sla`。

## #32 Vec 值参跨语句复用搬移 trap（已确认语义，2026-10-10，JEV 修票根因覆写记档）

- 现象：`fn vec_eq(a: Vec<i32>, b: Vec<i32>)` 连调两次复用首参，
  check/test 双绿，exe 构建期报 `UseAfterMove`（`tmp_21`）；
  单调用 + 无复用双调用三线绿；阳性对照 `take(e1)` 后 `len(e1)`
  同样 check 过、test 过、exe 构建败（`tmp_2`）。
- 根因（已钉死，非 lowering bug）：Vec 值参即搬移（与 127 的数组借入
  不同），跨语句复用本就违规；39 的循环体内复用系分析未覆盖才放行
  （勿仿）；JEV 单选曾裁 fix_now（64%，低置信），根因探明后复议——
  收紧循环侧必断 39、放松序列侧必坏所有权，两向皆坏，故记档（#24 先例）。
- 现状规避：Vec 比较以内联写（253 口径）；值参调用后不再碰原绑定。
- 复现探针：`probe_vecmore.sla`（三调版，exe 败）、`probe_veceq3.sla`
  （复用版 trap / 无复用版绿）、`probe_vecmove.sla`（阳性对照）、
  `probe_veceq2.sla`（内联版三线绿）。

## #33 容器装结构体值 exe 不支持（已确认缺口，2026-10-10，JEV 裁定记档绕行）

- 现象：`HashMap<str, Pt>` 存取可过 check；`unwrap_or_default` 版 test
  双绿但 exe 报 `UnsupportedSabDirectFeature`；`match` 提取版 SAB 亦败、
  SA 报 `MemoryLeak`（`tmp_9`）；Vec 内结构体读写（223/271）与枚举表
  （272）三线全绿（容器装值本身可用，map 侧 direct lowering 缺失）。
- 定性：与 #48 同属值格容器类；JEV 单选 record 85%。
- 现状规避：容器只装标量（253 口径）；map-struct 类用例暂略。
- 复现探针：`probe_combos.sla`（三分）、`probe_c1.sla`（vecstruct 写绿）、
  `probe_c2.sla`（vecenum 绿）、`probe_c3.sla`（mapstruct exe 败）、
  `probe_c4.sla`（match 版双端败）。

## #34 JS 后端 SAB 有 dest 调用报 InvalidOperand（已修复，2026-10-13，sci 侧）

- 现象：`sa sla build-wasm` 全绿，但 `sa sla build-js` 几乎全败
  （抽查 1/11，仅裸字符串 `println("hello world")` 通过）：
  任何 `println("{}", x)` 报 `JS backend emit failed: InvalidOperand in main`；
  同一 SAB 直调 `sa build-js` 同错，`.sa` 文本路径反绿。
- 根因：SAB 结构化调用操作数为 `[dest.reg, body.text]` 且 `raw_text` 为空；
  `sci/src/emit_js.zig` 的 `emitCallInstruction` /
  `emitCallIndirectInstruction` 把 dest 经全局符号表做文本往返
  （`lookupName(reg)` → 别名 → `findId` → `slotOf`），函数局部槽位被
  误解析为无关全局名（实测 `dest=base id=4`，`slotOf` 失败）。
  其余全部指令（含 wasm/LLVM）均经 `dstSlot` 直解寄存器操作数，
  唯两处 call 发射例外，故 native/wasm 正常、仅 JS 败。
- 已修复：两处 call-dest 改走 `dstSlot(fsig, use_global, base.operands[0])`
  直解（`.reg`/`.symbol` 时），仅非寄存器形态回落文本查找；
  并删去因此冗余的 `_ = use_global;`。探针 `/tmp/pvar.sla` 的 SAB
  现 `build-js` 通过且 `node` 输出 `14` 正确；`02_arith` 的 JS 输出
  `14/4/2/-5` 与 `@test` 断言一致；抽查 11/11 全绿。
- 归属：sci JS 后端（`sci/src/emit_js.zig`），SLA 插件侧仅新增透传命令。

## 已验证无问题（回归对照）

- [x] `Vec<i32>` vs `Vec<int>` stride（09_vec_methods 全绿，无数组字面量式问题）。
- [x] `switch` over `i32`（13_enum_switch 全绿）。
- [x] `Option::Some/None + match`（15_option 全绿，未触发 PhiStateConflict）。
- [x] 闭包捕获 + 循环（20_closures 全绿）。
- [x] `while true + break/continue`（32 全绿）、`do-while` 语义经 `break` 表达（40 全绿）。
- [x] 嵌套循环众数/中位数排序（37/38 全绿）、`Vec` 拼接去重（39 全绿）。
- [x] 归并/位运算/素数上限/默认参数双函数/label 改写（41-48 全绿）。
- [x] 长循环 collatz（49 全绿，步数口径已用 build-exe 核对为 111）、
  嵌套 `if` + 步长 `while`（50 全绿；`for` 暂无步长语法，用 `while` 表达）。
