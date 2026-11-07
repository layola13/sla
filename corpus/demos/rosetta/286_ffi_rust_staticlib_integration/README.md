# 286 Ffi Rust Staticlib Integration

This slot now uses a real fixture-backed Rust staticlib FFI integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/rust_static_gate.*`, `ffi/rust_staticlib.sai`, Cargo bridge metadata, header, and archive note.
- `main.sla`: current surrogate that only preserves the Rust staticlib symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/286_ffi_rust_staticlib_integration/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/286_ffi_rust_staticlib_integration/main.sla --out /tmp/286_ffi_rust_staticlib_integration.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/286_ffi_rust_staticlib_integration/main.sla
```
