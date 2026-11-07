# 102 Raii Guard

This slot models scoped mutex guard acquisition, early return, and guarded update behavior.

- `main.rs`: Rust reference for scoped mutex guard acquisition, early return, and guarded update behavior.
- `main.sla`: Sla companion for the same guard path, with minimal dereference staging that is currently required for the checked update path to type-check locally.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/102_raii_guard/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/102_raii_guard/main.sla --out /tmp/102_raii_guard.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/102_raii_guard/main.sla
```
