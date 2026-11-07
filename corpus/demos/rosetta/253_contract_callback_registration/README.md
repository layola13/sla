# 253 Contract Callback Registration

This slot now uses a real fixture-backed callback-registration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one callback count instead of checking the vtable slot, indirect call, and registration consumer.

- `main.rs`: Rust reference that reads `bridge/callback_vtable.sa` and `consumer/callback_consumer.sa`.
- `main.sla`: current surrogate that only preserves the one-callback count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/253_contract_callback_registration/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/253_contract_callback_registration/main.sla --out /tmp/253_contract_callback_registration.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/253_contract_callback_registration/main.sla
```
