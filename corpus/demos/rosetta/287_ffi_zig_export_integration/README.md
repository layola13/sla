# 287 Ffi Zig Export Integration

This slot now uses a real fixture-backed Zig export FFI integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `guest/zig_export.*` and `host/zig/*` export bridge files.
- `main.sla`: current surrogate that only preserves the exported-symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/287_ffi_zig_export_integration/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/287_ffi_zig_export_integration/main.sla --out /tmp/287_ffi_zig_export_integration.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/287_ffi_zig_export_integration/main.sla
```
