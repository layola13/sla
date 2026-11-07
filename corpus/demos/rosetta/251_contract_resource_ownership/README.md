# 251 Contract Resource Ownership

This slot now uses a real fixture-backed ownership-transfer reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one transfer count instead of checking the extern, mutating bridge, and handle-passing consumer.

- `main.rs`: Rust reference that reads `iface/ownership.sai`, `bridge/ownership_bridge.sa`, and `consumer/ownership_consumer.sa`.
- `main.sla`: current surrogate that only preserves the moved-resource count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/251_contract_resource_ownership/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/251_contract_resource_ownership/main.sla --out /tmp/251_contract_resource_ownership.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/251_contract_resource_ownership/main.sla
```
