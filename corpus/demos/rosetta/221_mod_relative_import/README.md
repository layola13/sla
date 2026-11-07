# 221 Mod Relative Import

This slot now uses a real relative-import fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a hop-count observable instead of true module path resolution.

- `main.rs`: Rust reference that reads `helper.sa`, `chain/step.sa`, and `chain/deeper/seed.sa` to check the relative import chain.
- `main.sla`: current surrogate that only preserves a one-hop count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/221_mod_relative_import/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/221_mod_relative_import/main.sla --out /tmp/221_mod_relative_import.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/221_mod_relative_import/main.sla
```
