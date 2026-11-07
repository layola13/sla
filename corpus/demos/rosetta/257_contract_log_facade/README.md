# 257 Contract Log Facade

This slot now uses a real fixture-backed log-facade reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves three log-level counts instead of checking the message-bearing ABI signature and consumer call.

- `main.rs`: Rust reference that reads `iface/log.sai`, `bridge/log_bridge.sa`, and `consumer/log_consumer.sa`.
- `main.sla`: current surrogate that only preserves the three-level count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/257_contract_log_facade/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/257_contract_log_facade/main.sla --out /tmp/257_contract_log_facade.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/257_contract_log_facade/main.sla
```
