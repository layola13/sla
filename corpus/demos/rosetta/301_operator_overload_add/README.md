# 301 Operator Overload - Binary Add (`Vec3 + Vec3`)

> **状态**：当前 Sla companion 已使用真实 `a + b` 操作符。编译器在类型检查阶段允许数值字段 struct 的同类型 `+`，在 codegen 阶段生成逐字段 `add`。

This directory documents the local `Vec3 + Vec3` operator-overload demo.

- `main.rs`：Rust 原版，演示 `impl Add for Vec3` 实现 `a + b`。
- `main.sla`：Sla 等价实现，显式导入 `sa_std/ops.sa`，在 `main` 路径直接使用 `f32` 字段和 `let c = a + b;`，验证结果为 `Vec3 { x: 5.0, y: 7.0, z: 9.0 }`。

## 命令

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/301_operator_overload_add/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/301_operator_overload_add/main.sla --out /tmp/301.sa
SA_PLUGIN_DEV=1 sa sla test  demos/rosetta/301_operator_overload_add/main.sla
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
    let b = Vec3 { x: 4.0, y: 5.0, z: 6.0 };
    let c = a + b;
    if c.x != 5.0 { return 0; };
    if c.y != 7.0 { return 0; };
    if c.z != 9.0 { return 0; };
    return 579;
}
```

## 编译器实现要点

1. **Type checker**：`binary_expr` 中同一 struct 的 `+` 要求所有字段是数值类型。
2. **Codegen**：为结果 struct 分配存储，逐字段 `load`、`add`、`store`。
3. **生成 SA**：不调用命名模拟 helper，直接生成字段级 `add`。
