# 246 Contract Semver Minor Update

This slot now uses a real fixture-backed semver-minor reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves an added-field count instead of checking the retained and added externs across iface/impl/consumer files.

- `main.rs`: Rust reference that reads `iface/minor.sai`, `impl/minor_impl.sa`, and `consumer/minor_consumer.sa`.
- `main.sla`: current surrogate that only preserves the one-added-field count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/246_contract_semver_minor_update/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/246_contract_semver_minor_update/main.sla --out /tmp/246_contract_semver_minor_update.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/246_contract_semver_minor_update/main.sla
```
