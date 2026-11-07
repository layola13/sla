# 258 Contract Thread Local Isolation

This slot now uses a real fixture-backed thread-local reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves two slot counts instead of checking TLS layout, bridge mutation, and consumer allocation.

- `main.rs`: Rust reference that reads `layout/tls.sal`, `bridge/tls_bridge.sa`, and `consumer/tls_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-slot count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/258_contract_thread_local_isolation/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/258_contract_thread_local_isolation/main.sla --out /tmp/258_contract_thread_local_isolation.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/258_contract_thread_local_isolation/main.sla
```
