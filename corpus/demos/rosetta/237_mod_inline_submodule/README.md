# 237 Mod Inline Submodule

This slot now uses a real nested submodule fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a layer-count observable instead of true inline submodule semantics.

- `main.rs`: Rust reference that reads the outer submodule, inline module, and deep seed module.
- `main.sla`: current surrogate that only preserves a one-layer count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/237_mod_inline_submodule/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/237_mod_inline_submodule/main.sla --out /tmp/237_mod_inline_submodule.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/237_mod_inline_submodule/main.sla
```
