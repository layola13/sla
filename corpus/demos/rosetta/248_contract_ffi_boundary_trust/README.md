# 248 Contract Ffi Boundary Trust

This slot now uses a real fixture-backed FFI-boundary reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves pointer/length check counts instead of modeling the `@ffi_wrapper` raw-pointer trust boundary.

- `main.rs`: Rust reference that reads the slot layout, `@ffi_wrapper` bridge, and raw-edge consumer.
- `main.sla`: current surrogate that only preserves the two-check count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/248_contract_ffi_boundary_trust/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/248_contract_ffi_boundary_trust/main.sla --out /tmp/248_contract_ffi_boundary_trust.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/248_contract_ffi_boundary_trust/main.sla
```
