# 252 Contract Error Code Mapping

This slot now uses a real fixture-backed error-code reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves two mapping counts instead of checking the branch-bearing bridge and consumer call.

- `main.rs`: Rust reference that reads `iface/error_codes.sai`, `bridge/error_map.sa`, and `consumer/error_consumer.sa`.
- `main.sla`: current surrogate that only preserves the two-mapping count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/252_contract_error_code_mapping/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/252_contract_error_code_mapping/main.sla --out /tmp/252_contract_error_code_mapping.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/252_contract_error_code_mapping/main.sla
```
