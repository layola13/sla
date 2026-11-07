# 163 Object Safety Rules

This directory matches the current local object-safety slot shape.

- `main.rs`: Rust reference for dyn dispatch through `render(&item)` over `trait Draw`.
- `main.sla`: Sla companion for the same current local trait-object dispatch shape.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/163_object_safety_rules/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/163_object_safety_rules/main.sla --out /tmp/163_object_safety_rules.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/163_object_safety_rules/main.sla
```
