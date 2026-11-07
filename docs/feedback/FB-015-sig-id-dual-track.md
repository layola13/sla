# FB-015 — genFuncSig 的 sig.id 与 decl 指令的 symbol id 双轨不一致

- **反馈环批次**：第三批（SAB 二期 B 组移植中发现）
- **源登记**：`docs/divergences/sab.md` SAB-D17
- **分类**：Zig 侧缺陷候选 —— 源码机制定案（未做全管线实证）
- **Rust 侧处置**：只镜像 + 登记。Rust 端口的发射主干（B5）以 intern id
  为 sig.id（与 phase-1 黄金向量口径一致），driver 不另立第二轨；
  不替 Zig 改行为。

## 最小输入

任意含两个以上函数的程序，例如：

```sla
fn add(a: i32, b: i32) -> i32 { return a + b; }
fn main() -> i32 { return add(40, 2); }
```

## 机制（Zig 源码位置，本机参照树行号）

- `src/sab_codegen.zig` `genFuncSig`（~6043）：`FunctionSig.id` 被赋为
  `@intCast(self.function_sigs.items.len + self.test_sigs.items.len)`——
  即「已发射签名条数」这一序号，与符号表无关。
- 同文件 `declInstForSig`（~6007-6021）：decl 指令的两个操作数
  （`.symbol` / `.func`）用的是 `intern(fsig.name)`——符号表 id。
- 于是同一函数在同一模块里有两个 id：sig 记录里是序号 id，
  decl 指令里是 intern id。单函数程序两者恰好都是 0，多函数程序
  即分叉（上例 `main`：sig.id = 1，decl 操作数 = intern("main")，
  其值取决于此前的 intern 顺序）。
- sci 侧若有任何消费者按 `sig.id` 去符号表查名字（或反之按 decl
  操作数匹配 sig），都会在多函数模块上错位。当前已知的 sci 消费
  者是否中招未实证——这正是本单要求主线复核的点。

## 建议修复形态（仅建议）

`genFuncSig` 的 id 改为 `intern(lowered)` 的返回值（与 decl 同轨），
或在 sci 侧明确 sig.id 的语义并加一条多函数模块的断言测试
（sig.id 与 decl 操作数恒等）。修复后差分复核口径：双函数模块的
SAB 字节中每个 sig 的 id 与其 decl 指令操作数一致。

## Rust 侧锚点

`crates/sla-codegen-sab/src/emit.rs::ModuleBuilder::add_sig`
（sig.id = intern(name)）；端到端证据
`crates/sla-codegen-sab/tests/driver_e2e.rs::driver_p1_matches_hand_model_byte_for_byte`。
