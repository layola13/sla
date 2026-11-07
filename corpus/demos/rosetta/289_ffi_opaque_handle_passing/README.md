# 289 Ffi Opaque Handle Passing

This slot now uses a real fixture-backed opaque handle FFI passing reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/handle_gate.*`, `ffi/handle.sai`, host handle header, ownership manifest, and notes.
- `main.sla`: current surrogate that only preserves the handle round-trip count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/289_ffi_opaque_handle_passing/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/289_ffi_opaque_handle_passing/main.sla --out /tmp/289_ffi_opaque_handle_passing.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/289_ffi_opaque_handle_passing/main.sla
```
