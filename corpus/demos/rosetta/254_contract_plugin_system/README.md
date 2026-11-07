# 254 Contract Plugin System

This slot now uses a real fixture-backed plugin-system reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves two enabled-plugin counts instead of checking host extern, implementation export, and consumer dispatch wiring.

- `main.rs`: Rust reference that reads `host/plugin_host.sa`, `impl/plugin_impl.sa`, and `consumer/plugin_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-plugin count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/254_contract_plugin_system/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/254_contract_plugin_system/main.sla --out /tmp/254_contract_plugin_system.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/254_contract_plugin_system/main.sla
```
