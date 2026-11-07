# 282 Ffi Link Static C Lib

This slot now uses a real fixture-backed static C library FFI linkage reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/static_gate.*`, `ffi/static_lib.sai`, static header, archive note, and linker script.
- `main.sla`: current surrogate that only preserves the static archive-member count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/282_ffi_link_static_c_lib/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/282_ffi_link_static_c_lib/main.sla --out /tmp/282_ffi_link_static_c_lib.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/282_ffi_link_static_c_lib/main.sla
```
