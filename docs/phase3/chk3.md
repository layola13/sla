# CHK3 线交付报告 — sla-checker 剩余 3 项（三期）

场地：Kaggle 场地三（2202）；真插件现头 sla `6002b386`；
本地主本先行、自家路径逐路径同步（擦除 → tar 管道 → 两侧
md5 聚合一致，三轮均一致）。不 git commit。

**真值口径注记**：本机参照树 `~/workspace/local-sa` 为
3de91932（type_checker.zig 6,098 行），场地现头同文件为
10,917 行——任务书「6,098 行」与场地现头不是同一树。本线以
**场地现头源码 + 真插件探针**为最终真值（行号均为现头）；
两树的三项机器经抽查一致（plain-if 臂、守卫臂逐字相同；
关联机器现头多 DEF-E0 的 using 定序注记，按现头实现）。

## 测试数字

- `cargo test -p sla-checker`（场地三）：**118 passed / 0 failed**
  （基线 80，新增 38，只增不减）。
- `cargo test --workspace --no-fail-fast`（场地三）：除
  sla-plugin 一件外全绿——`contracts_pipeline` 的
  `println_remains_blocked_by_missing_checker_builtin_arm`
  1 红，根因为 D-C18（见下），属 INT-2 线测试的文案 pin 钉了
  本线按 Zig 对齐前的旧回退串，该线文件不在本线辖区、未动，
  候协调线转达该线更新 pin（println 仍被阻断的意图不变）。
- 全部 .rs ≤700 行（最大 check_method.rs 636、check_call.rs
  654、tests.rs 582）。lib.rs mod 声明变更：新增
  `mod check_method;`、测试 mod `tests_phase3` /
  `tests_phase3b` / `tests_phase3c`。Cargo.toml 依赖边未动。

## 逐项销账

### 1. 关联调用接收者类型推导全形态 ✔

Zig 真值：31 件探针中的关联组 16 件（`sa sla check`，rc +
诊断原文），关键项：固有点调用/关联调用 rc=0；`&self` 裸
接收者 rc=0；`&T` 与 `*T` 类型接收者调 plain-self 方法
rc=1 TypeMismatch；trait 点调用/`Trait::m(&x)`/
`Type::build(..)` rc=0；固有优先于 trait（bool/i32 返回型
判别探针 rc=0）；双 trait 同名 rc=1「Ambiguous trait method
`m` for type `S`」(TypeMismatch)；using 扩展 rc=0、无 using
rc=1「Undefined call: double」、双 using rc=1「Ambiguous
static extension call `double` for type `primitive`」
(AmbiguousCall)、借用型首参扩展 rc=0；错接收者/未知关联
rc=1「Undefined call: {name}」(UndefinedVariable)；
`Option::None()` rc=0。

改动：新建 `check_method.rs`（resolve_trait_method_symbol
全量移植、using resolver + extension 调用检查、
check_method_args = checkCallArgsAgainstSignature 忠实移植、
check_receiver_call 固有→trait→dyn→using 定序）；
check_call.rs 关联形重写（trait-target / 固有 funcs-only /
Option::None / trait 回退 funcs-only + miss 回落后续解析级）
+ 接收者级接入分派尾 + 未解析回退对齐 Zig（D-C18）；
checker.rs FuncSig 加 is_pub + current_using_modules；
scope.rs Frame 加 using_modules；check_program.rs 全局
using 登记；check_stmt.rs 作用域 using 登记；
types_util.rs 加 normalize_using_path / is_string_type /
type_tag_name；check_literal.rs 删一处冗余 import（既有
unused-import 警告，本 crate 内顺手清）。

新增测试 20（tests_phase3b 17 + tests_phase3c 3）：parser
切片不覆盖 trait/impl 声明，impl/trait 形态以 AST 经
check_program 钉（同二期 trait 测试形态），mod/using 与
Option::None 走源码全管线；每条标注对应场地探针。

### 2. match 守卫绑定可见性/消费语义 ✔（核清为结构等价）

Zig 真值（10 探针）：守卫见图案绑定 rc=0；守卫 `^v` 消费
图案绑定后体复用 rc=1 UseAfterMove（Option 路与 enum 路
各一）；守卫 try 消费外层绑定后体复用 rc=1、且 match 后
复用 rc=1（守卫效应进入 case 活状态参与多路合并——
**检查器无守卫失败回滚**，守卫与体同序同账）；守卫单独
消费 rc=0 对照；跨 case 绑定 rc=1 UndefinedVariable；
图案绑定不逃逸 rc=1；守卫 i32/字面量 rc=1 TypeMismatch。

结论：二期「同帧近似」与 Zig pattern_scope 结构（图案作用域
为体块作用域之父、状态共享同一符号对象）在状态语义上
等价——**无代码改动**，以 tests_phase3.rs 中 9 件探针形态
全管线单测钉死，防后续线误「补回滚」。

### 3. plain-if 条件接受 integer ✔

Zig 真值（9 探针）：bool rc=0；比较表达式 rc=0；整数字面量
rc=1；i32 变量 rc=1；`integer` 注解变量 rc=1；`int` 注解
变量 rc=1；f64 字面量 rc=1；String 变量 rc=1。即 Zig 臂
（boolean / legacy integer / infer）中，legacy integer 在
源码层实际只由比较表达式产生（注解与字面量产出的都是
定宽类型）——此为注解层事实，已登记备查、非检查器分歧。

改动：check_flow.rs 条件判定改 Zig 原形三形态。
新增测试 9（tests_phase3.rs）：源码全管线 6（bool/比较/
字面量/i32/f64/String）+ AST 直钉 legacy-integer 与 infer
两形态各 1（源码层不可达、按 INT-1 教训注明形态来源）。

## 新分歧与反馈单

- 新登记 D-C18…D-C21（docs/divergences/checker.md 三期节）：
  D-C18 未解析回退已对齐（本移植缺陷已修，含 INT-2 跨线
  pin 注记）；D-C19 接收者级分派位置差（剩余近似）；
  D-C20 FuncSig 无 is_async（方法返回不包 future）；
  D-C21 方法路径侧表/强转三件缺口。
- 仍剩余近似（确切清单见 checker.md 三期节）：std 叶级联
  （Option/Vec/HashMap 等方法叶、push/pop/remove、
  iter/map/filter/fold/collect/join 叶）、静态构造叶
  （Vec::new 等）、str::from_utf8 叶、plain 路实参检查一期
  简化形与方法路忠实形并存。
- **反馈单：无新增**。三项 Zig 行为探针核清后均自洽；
  号段 FB-029..FB-032 全部未用，归还协调线。

## 改动文件清单

- crates/sla-checker/src/check_method.rs（新，636 行）
- crates/sla-checker/src/check_call.rs（关联重写 + 接收者级
  接入 + 回退对齐）
- crates/sla-checker/src/check_flow.rs（plain-if 三形态）
- crates/sla-checker/src/checker.rs（FuncSig.is_pub +
  current_using_modules）
- crates/sla-checker/src/scope.rs（Frame.using_modules）
- crates/sla-checker/src/check_program.rs（全局 using 登记）
- crates/sla-checker/src/check_stmt.rs（作用域 using 登记）
- crates/sla-checker/src/types_util.rs（using 规范化等三助手）
- crates/sla-checker/src/check_literal.rs（冗余 import 清理）
- crates/sla-checker/src/lib.rs（mod 声明，见上）
- crates/sla-checker/src/tests.rs（FuncSig 字面量补 is_pub
  一行）
- crates/sla-checker/src/tests_phase3.rs（新，18 测）
- crates/sla-checker/src/tests_phase3b.rs（新，17 测）
- crates/sla-checker/src/tests_phase3c.rs（新，3 测）
- docs/divergences/checker.md（三期销账节）
