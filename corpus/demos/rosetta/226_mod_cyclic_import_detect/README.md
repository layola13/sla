# 226 Mod Cyclic Import Detect

This slot now uses a real cyclic-import fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a diagnostic count instead of true cycle rejection.

- `main.rs`: Rust reference that reads the `cycle/core` self-cycle and the sibling `alpha/beta/gamma` cycle fixtures.
- `main.sla`: current surrogate that only preserves a one-cycle diagnostic count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/226_mod_cyclic_import_detect/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/226_mod_cyclic_import_detect/main.sla --out /tmp/226_mod_cyclic_import_detect.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/226_mod_cyclic_import_detect/main.sla
```
