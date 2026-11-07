# 224 Mod Reexport Pub Use

This slot now uses a real re-export-style bridge fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a re-export count instead of true `pub use` semantics.

- `main.rs`: Rust reference that reads the bridge module, deep value module, and hidden seed module.
- `main.sla`: current surrogate that only preserves a one-reexport count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/224_mod_reexport_pub_use/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/224_mod_reexport_pub_use/main.sla --out /tmp/224_mod_reexport_pub_use.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/224_mod_reexport_pub_use/main.sla
```
