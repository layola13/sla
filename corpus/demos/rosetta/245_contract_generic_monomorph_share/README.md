# 245 Contract Generic Monomorph Share

This slot now uses a real fixture-backed shared-contract reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves two call-path counts instead of checking iface/impl/consumer symbol sharing.

- `main.rs`: Rust reference that reads `iface/generic.sai`, `impl/generic_impl.sa`, and `consumer/generic_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-path count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/245_contract_generic_monomorph_share/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/245_contract_generic_monomorph_share/main.sla --out /tmp/245_contract_generic_monomorph_share.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/245_contract_generic_monomorph_share/main.sla
```
