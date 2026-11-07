# 241 Contract Layout Stability

This slot now uses a real fixture-backed contract layout reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a two-field count observable instead of importing and enforcing the `layout/point.sal` plus bridge/consumer contract graph.

- `main.rs`: Rust reference that reads `layout/point.sal`, `bridge/point_bridge.sa`, and `consumer/point_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-field count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/241_contract_layout_stability/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/241_contract_layout_stability/main.sla --out /tmp/241_contract_layout_stability.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/241_contract_layout_stability/main.sla
```
