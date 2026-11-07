# 233 Mod Alias Import

This slot now uses a real alias-style module fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves an alias-count observable instead of true module aliasing.

- `main.rs`: Rust reference that reads the alias wrapper, deep module, and seed module.
- `main.sla`: current surrogate that only preserves a one-alias count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/233_mod_alias_import/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/233_mod_alias_import/main.sla --out /tmp/233_mod_alias_import.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/233_mod_alias_import/main.sla
```
