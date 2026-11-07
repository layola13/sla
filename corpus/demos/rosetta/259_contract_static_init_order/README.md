# 259 Contract Static Init Order

This slot now uses a real fixture-backed static-init reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves two stage counts instead of checking the ordered bridge calls and layout stores.

- `main.rs`: Rust reference that reads `layout/init_order.sal`, `bridge/init_bridge.sa`, and `consumer/init_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-stage count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/259_contract_static_init_order/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/259_contract_static_init_order/main.sla --out /tmp/259_contract_static_init_order.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/259_contract_static_init_order/main.sla
```
