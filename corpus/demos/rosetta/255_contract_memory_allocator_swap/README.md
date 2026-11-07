# 255 Contract Memory Allocator Swap

This slot now uses a real fixture-backed allocator-swap reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one allocator-selection count instead of checking the allocator extern, bridge mutation, and consumer handle flow.

- `main.rs`: Rust reference that reads `iface/allocator.sai`, `bridge/allocator_bridge.sa`, and `consumer/allocator_consumer.sa`.
- `main.sla`: current surrogate that only preserves the alternate-allocator count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/255_contract_memory_allocator_swap/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/255_contract_memory_allocator_swap/main.sla --out /tmp/255_contract_memory_allocator_swap.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/255_contract_memory_allocator_swap/main.sla
```
