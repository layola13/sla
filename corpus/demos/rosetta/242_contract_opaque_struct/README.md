# 242 Contract Opaque Struct

This slot now uses a real fixture-backed opaque-struct reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a handle-count observable instead of separating public and private layout contracts.

- `main.rs`: Rust reference that reads the public/private layout split, bridge, and consumer files.
- `main.sla`: current surrogate that only preserves the public-constructor count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/242_contract_opaque_struct/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/242_contract_opaque_struct/main.sla --out /tmp/242_contract_opaque_struct.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/242_contract_opaque_struct/main.sla
```
