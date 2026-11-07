# 304 Operator Overload - Custom Eq (`a == b` on struct)

> **状态**：当前 Sla companion 已使用真实 `a == b` / `a != c` 操作符。struct 需要显式 `@derive(PartialEq)`（对应 Rust 的 `impl PartialEq`）；编译器在类型检查阶段允许带该 derive 且字段可比较的同类型 struct 做 `==`/`!=`，在 codegen 阶段生成逐字段 `eq` 与 `and`。

This directory documents the local `Point == Point` / `Point != Point` operator-overload demo.

- `main.rs`：Rust 原版，演示 `impl PartialEq for Point` 实现 `a == b` 和 `a != c`。
- `main.sla`：Sla 等价实现，在 `main` 路径使用 `let` 绑定的 `Point` 直接比较，验证 `a == b` 且 `a != c`。

## 命令

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/304_operator_overload_eq/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/304_operator_overload_eq/main.sla --out /tmp/304.sa
SA_PLUGIN_DEV=1 sa sla test  demos/rosetta/304_operator_overload_eq/main.sla
```

## 当前 Sla 示例

```sla
@derive(PartialEq)
struct Point {
    x: i32,
    y: i32,
}

fn main() -> i32 {
    let a = Point { x: 10, y: 20 };
    let b = Point { x: 10, y: 20 };
    let c = Point { x: 99, y: 0 };

    if a == b {
        if a != c { return 11; } else { return 0; };
    } else {
        return 0;
    };
}
```

## 编译器实现要点

1. **Type checker**：`==` / `!=` 要求左右是同一 struct、struct 带 `@derive(PartialEq)`（或 `@derive(eq)`），且字段都是可比较 primitive。
2. **Codegen**：逐字段 `load`、`eq`，再用 `and` 合并；`!=` 由合并结果反转得到。
3. **生成 SA**：不调用命名模拟 helper，直接生成字段级 `eq` / `and`。
