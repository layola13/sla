# 312 Operator Overload Block

> **状态**：展示 `@overload` 块内的静态 `+` 分发；前端将 `a + b` 降级为同类型静态函数调用，不引入运行时分发。

This demo shows the explicit `@overload` block form used by the Sla frontend.

- `main.rs`: Rust reference.
- `main.sla`: Sla version with `@overload Vec2 { fn +(self: Vec2, other: Vec2) -> Vec2 { ... } }` and `a + b`.

## Commands

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/312_operator_overload_block/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/312_operator_overload_block/main.sla --out /tmp/312.sa
SA_PLUGIN_DEV=1 sa sla test  demos/rosetta/312_operator_overload_block/main.sla
```
