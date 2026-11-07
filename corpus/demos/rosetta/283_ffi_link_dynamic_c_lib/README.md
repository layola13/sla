# 283 Ffi Link Dynamic C Lib

This slot now uses a real fixture-backed dynamic C library FFI linkage reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/dynamic_gate.*`, `ffi/dynamic_lib.sai`, dynamic header, rpath notes, and pkg-config file.
- `main.sla`: current surrogate that only preserves the dynamic symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/283_ffi_link_dynamic_c_lib/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/283_ffi_link_dynamic_c_lib/main.sla --out /tmp/283_ffi_link_dynamic_c_lib.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/283_ffi_link_dynamic_c_lib/main.sla
```
