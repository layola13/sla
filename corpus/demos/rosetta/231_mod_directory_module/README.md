# 231 Mod Directory Module

This slot now uses a real directory-backed fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a route-count observable instead of true directory-module resolution.

- `main.rs`: Rust reference that reads `module/index.sa`, `module/tree/index.sa`, and the leaf modules beneath them.
- `main.sla`: current surrogate that only preserves a two-route count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/231_mod_directory_module/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/231_mod_directory_module/main.sla --out /tmp/231_mod_directory_module.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/231_mod_directory_module/main.sla
```
