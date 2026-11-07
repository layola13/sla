# 222 Mod Absolute Import

This slot now uses a real absolute-looking module-root fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a segment-count observable instead of true crate-root import resolution.

- `main.rs`: Rust reference that reads `shared/root/index.sa`, `shared/root/codec/index.sa`, and the leaf module.
- `main.sla`: current surrogate that only preserves a two-segment count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/222_mod_absolute_import/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/222_mod_absolute_import/main.sla --out /tmp/222_mod_absolute_import.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/222_mod_absolute_import/main.sla
```
