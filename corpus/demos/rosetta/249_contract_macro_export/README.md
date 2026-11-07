# 249 Contract Macro Export

This slot now uses a real fixture-backed macro-export reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a macro-count observable instead of checking macro definition/import/expansion structure.

- `main.rs`: Rust reference that reads `macros/store.sa`, `bridge/macro_bridge.sa`, and `consumer/macro_consumer.sa`.
- `main.sla`: current surrogate that only preserves the one-macro count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/249_contract_macro_export/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/249_contract_macro_export/main.sla --out /tmp/249_contract_macro_export.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/249_contract_macro_export/main.sla
```
