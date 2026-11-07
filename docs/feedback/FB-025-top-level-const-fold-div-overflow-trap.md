# FB-025 — 顶层常量折叠对 i64::MIN / -1 的除法/取余无防护，编译器被 SIGFPE 击穿

- **反馈环批次**：三期 SAB-C 线（C1 prescan_consts 移植中发现并实证）
- **源登记**：`docs/divergences/sab.md` SAB-D25
- **分类**：Zig 侧缺陷候选 —— **真产物实证**（场地三现头插件，
  文本路与 SAB 路双双核心转储）
- **Rust 侧处置**：折叠以 `checked_div` / `checked_rem` 处理，
  该形态**放弃折叠**（返回 null，与既有 add/sub/mul 溢出
  `catch return null` 的口径一致），不复刻陷阱；本单登记候
  主线修复。

## 最小输入

```sla
const A1: int = 0 - 9223372036854775807;
const A2: int = A1 - 1;
const NEG: int = 0 - 1;
const B: int = A2 / NEG;

fn main() -> int {
    return B;
}
```

（A2 经两步合法折叠得到 i64::MIN，NEG 折叠得 -1，B 的折叠即
MIN / -1。取余形态 `A2 % NEG` 同理。）

## 实证（2026-11-07，场地三 Kaggle 2202，现头 sla 6002b386）

- `sa sla sab build divmin.sla` → **rc=136，SIGFPE，core dumped**
  （timeout 监视下报 Floating point exception），无任何诊断。
- `sa sla build divmin.sla`（文本路）→ **rc=136，同样 SIGFPE**。
  两路同崩，说明陷阱在共享的整常量折叠层，不止 SAB 一处。
- 对照组：`3 << 62` 的移位折叠两路均正常（折叠值为位模式
  -4611686018427387904），`ri == 0` 的除法亦被既有防护挡下；
  缺口仅在 MIN / -1 这一溢出点。

## 机制（Zig 源码位置，本机参照树行号）

- `src/sab_codegen.zig` `foldTopLevelBinaryConst`（785–835）：
  int 臂 `.div => if (ri == 0) return null else @divTrunc(li, ri)`、
  `.mod => if (ri == 0) return null else @rem(li, ri)` —— 只防
  除零，不防 MIN / -1 溢出；同函数 add/sub/mul 已用
  `std.math.add/sub/mul(...) catch return null` 防护，口径不一。
- `src/lowering_rules.zig` `intConstantExprValue` 的 binary 臂
  同形：`.div/.mod` 仅判 `right == 0`，其余同上（文本路同崩
  的机制来源；确切行号候修复线定位时复核）。

## 建议修复形态（仅建议）

两处折叠点对 div/mod 增加溢出防护：`li == std.math.minInt(i64)
and ri == -1` 时返回 null（放弃折叠、回退正常发射，与既有
溢出口径一致），或改用 `std.math.divTrunc` / 相应 checked 形式
`catch return null`。修复后复核口径：上列输入两路均不再崩溃
（折叠放弃后由运行时/后续层处理），既有 const 折叠语料
（test_unit_toplevel_const_*）逐字不变。

## Rust 侧锚点

`crates/sla-codegen-sab/src/prescan_consts.rs::fold_top_level_binary_const`
（checked_div/checked_rem 处注释标明本单号）。
