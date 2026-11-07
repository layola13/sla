# 281 Ffi Link System Libc

This slot now uses a real fixture-backed system-libc FFI linkage reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/libc_gate.*`, `ffi/libc.sai`, `host/libc.h`, linker notes, and `system-libc.pc`.
- `main.sla`: current surrogate that only preserves the system-libc symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/281_ffi_link_system_libc/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/281_ffi_link_system_libc/main.sla --out /tmp/281_ffi_link_system_libc.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/281_ffi_link_system_libc/main.sla
```
