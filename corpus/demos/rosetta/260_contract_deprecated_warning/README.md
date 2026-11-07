# 260 Contract Deprecated Warning

This slot now uses a real fixture-backed deprecation reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one deprecated-entry count instead of checking the legacy note, extern, and consumer call.

- `main.rs`: Rust reference that reads `iface/deprecated.sai`, `bridge/deprecated_bridge.sa`, and `consumer/deprecated_consumer.sa`.
- `main.sla`: current surrogate that only preserves the one-entry count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/260_contract_deprecated_warning/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/260_contract_deprecated_warning/main.sla --out /tmp/260_contract_deprecated_warning.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/260_contract_deprecated_warning/main.sla
```
