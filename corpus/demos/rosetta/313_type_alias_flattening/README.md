# 313 Type Alias Flattening

> **状态**：展示 `type BulletData = Transform & Velocity & { damage: i32 };` 这种前端别名组合；编译器在 AST/type-check 阶段将其展开为扁平字段布局。

This demo shows the frontend-only alias flattening path used by the Sla compiler.

- `main.sla`: Sla version that composes a flattened `BulletData` and accesses fields directly.

## Commands

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/313_type_alias_flattening/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/313_type_alias_flattening/main.sla --out /tmp/313.sa
SA_PLUGIN_DEV=1 sa sla test  demos/rosetta/313_type_alias_flattening/main.sla
```
