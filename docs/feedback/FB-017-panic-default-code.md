# FB-017 — genCall 的 panic 臂对非单参数形态静默写死代码 "1"

- **反馈环批次**：第三批（SAB 二期 B 组移植中发现）
- **源登记**：`docs/divergences/sab.md` SAB-D22
- **分类**：Zig 侧缺陷候选 —— 源码机制定案（静默兜底族）
- **Rust 侧处置**：driver 字节级镜像该行为（0 参/多参 → 代码 1），
  同时在本单登记；不擅自改语义。

## 最小输入

```sla
fn f() -> i32 { panic(); }
fn g() -> i32 { panic(101, 202); }
```

两处调用的参数个数都不是 1，均被静默发射为 `panic` 代码文本
`"1"`——第二处显式给出的 101/202 被整体丢弃，无任何诊断。

## 机制（Zig 源码位置，本机参照树行号）

- `src/sab_codegen.zig` `genCall` 的 panic 臂（15167–15180）：三个
  分支为「单参数且字面量 → 其十进制文本」「单参数 → 代码寄存器」
  与 `else` 兜底 `item.operands[0] = .{ .text = "1" }`。
- 兜底臂不区分「无参数」与「多参数」，也不报错：多参数时实参
  表达式根本不被求值（求值副作用一并丢失），panic 代码被替换
  为 1。与 SAB-D14 的静默兜底家族同科。

## 建议修复形态（仅建议）

参数个数 != 1 时返回 `error.UnsupportedSabDirectFeature`（或在
检查器层先行拒绝 panic 的非法元数），使错误在编译期响亮暴露；
0 参数若确为合法简写，应在 AST/检查器层显式脱糖为 `panic(1)`
而非在 codegen 兜底。修复后复核口径：上列两输入均响亮报错，
单参数两形态字节不变。

## Rust 侧锚点

`crates/sla-codegen-sab/src/driver_gen.rs::gen_call` 的 panic 臂
（注释标明镜像来源与本单号）。
