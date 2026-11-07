# 247 Contract Semver Major Break

This slot now uses a real fixture-backed semver-major reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a removed-field count instead of checking the incompatible v1/v2 extern signature change.

- `main.rs`: Rust reference that reads `iface/v1.sai`, `iface/v2.sai`, `bridge/major_break_impl.sa`, and `consumer/major_consumer.sa`.
- `main.sla`: current surrogate that only preserves the one-break count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/247_contract_semver_major_break/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/247_contract_semver_major_break/main.sla --out /tmp/247_contract_semver_major_break.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/247_contract_semver_major_break/main.sla
```
