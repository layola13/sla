# 180 Try Trait V2

This directory now records the current `Try` / residual gap honestly.

- `main.rs`: Rust reference using `?` inside `fn add_one(...) -> Option<i32>`.
- `main.sla`: Sla surrogate that keeps the same `Option` flow through an explicit `match` helper.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/180_try_trait_v2/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/180_try_trait_v2/main.sla --out /tmp/180_try_trait_v2.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/180_try_trait_v2/main.sla
```
