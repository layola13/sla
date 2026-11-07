# 250 Contract Const Export

This slot now uses a real fixture-backed const-export reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one exported-const count instead of checking the iface declaration, implementation constant, and consumer call.

- `main.rs`: Rust reference that reads `iface/consts.sai`, `impl/const_impl.sa`, and `consumer/const_consumer.sa`.
- `main.sla`: current surrogate that only preserves the one-const count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/250_contract_const_export/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/250_contract_const_export/main.sla --out /tmp/250_contract_const_export.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/250_contract_const_export/main.sla
```
