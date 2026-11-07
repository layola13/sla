# 303 Operator Overload - Scalar Mul (`Vec3 * f32`)

> **状态**：当前 Sla companion 已使用真实 `a * 4.0` 操作符。编译器在类型检查阶段允许数值字段 struct 与数值标量相乘，在 codegen 阶段生成逐字段 `mul`。

This directory documents the local `Vec3 * f32` operator-overload demo.

- `main.rs`：Rust 原版，演示 `impl Mul<f32> for Vec3` 实现 `a * 4.0`。
- `main.sla`：Sla 等价实现，显式导入 `sa_std/ops.sa`，在 `main` 路径直接使用 `f32` 字段和 `let b = a * 4.0;`，验证结果为 `Vec3 { x: 4.0, y: 8.0, z: 12.0 }`。

## 命令

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/303_operator_overload_scalar_mul/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/303_operator_overload_scalar_mul/main.sla --out /tmp/303.sa
SA_PLUGIN_DEV=1 sa sla test  demos/rosetta/303_operator_overload_scalar_mul/main.sla
```

## 当前 Sla 示例

```sla
@import "sa_std/ops.sa"

struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

fn main() -> i32 {
    let a = Vec3 { x: 1.0, y: 2.0, z: 3.0 };
    let b = a * 4.0;
    if b.x != 4.0 { return 0; };
    if b.y != 8.0 { return 0; };
    if b.z != 12.0 { return 0; };
    return 4812;
}
```

## 编译器实现要点

1. **Type checker**：`struct * scalar` 和 `scalar * struct` 要求 struct 字段全是数值类型，标量也是数值类型。
2. **Codegen**：标量只生成一次，结果 struct 逐字段 `load`、`mul`、`store`。
3. **生成 SA**：不调用命名模拟 helper，直接生成字段级 `mul`。
