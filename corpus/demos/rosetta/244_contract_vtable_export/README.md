# 244 Contract Vtable Export

This slot now uses a real fixture-backed vtable reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a slot-count observable instead of constructing the exported vtable and indirect-call consumer path.

- `main.rs`: Rust reference that reads `button_vtable.sa` and `vtable_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-slot count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/244_contract_vtable_export/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/244_contract_vtable_export/main.sla --out /tmp/244_contract_vtable_export.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/244_contract_vtable_export/main.sla
```
