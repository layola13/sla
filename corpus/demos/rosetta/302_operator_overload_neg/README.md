# 302 Operator Overload - Unary Neg (`-vec`)

> **状态**：当前 Sla companion 已使用真实 `-a` 操作符。Parser 仍把一元负号表示为 `0 - expr`，类型检查和 codegen 已支持该形态作用于数值字段 struct。

This directory documents the local unary `-Vec3` operator-overload demo.

- `main.rs`：Rust 原版，演示 `impl Neg for Vec3` 实现 `-a`。
- `main.sla`：Sla 等价实现，显式导入 `sa_std/ops.sa`，在 `main` 路径直接使用 `f32` 字段和 `let b = -a;`，验证结果为 `Vec3 { x: -1.0, y: 2.0, z: -3.0 }`。

## 命令

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/302_operator_overload_neg/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/302_operator_overload_neg/main.sla --out /tmp/302.sa
SA_PLUGIN_DEV=1 sa sla test  demos/rosetta/302_operator_overload_neg/main.sla
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
    let a = Vec3 { x: 1.0, y: -2.0, z: 3.0 };
    let b = -a;
    if b.x != -1.0 { return 0; };
    if b.y != 2.0 { return 0; };
    if b.z != -3.0 { return 0; };
    return 123;
}
```

## 编译器实现要点

1. **Type checker**：`0 - struct` 识别为 struct 取负，要求所有字段是数值类型；`0 - float` 也作为浮点负号通过。
2. **Codegen**：逐字段 `load` 后生成 `sub 0.0, field` 并 `store` 到结果 struct。
3. **生成 SA**：不调用命名模拟 helper，直接生成字段级 `sub`。
