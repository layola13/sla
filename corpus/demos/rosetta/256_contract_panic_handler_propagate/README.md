# 256 Contract Panic Handler Propagate

This slot now uses a real fixture-backed panic-handler reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves two handler-hop counts instead of checking the iface, host hook, and consumer route.

- `main.rs`: Rust reference that reads `iface/panic.sai`, `host/panic_handler.sa`, and `consumer/panic_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-hop count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/256_contract_panic_handler_propagate/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/256_contract_panic_handler_propagate/main.sla --out /tmp/256_contract_panic_handler_propagate.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/256_contract_panic_handler_propagate/main.sla
```
