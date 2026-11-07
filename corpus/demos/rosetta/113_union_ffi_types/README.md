# 113 Union Ffi Types

This directory now records the current union-FFI surrogate honestly.

- `main.rs`: Rust reference for a `#[repr(C)]` union payload read.
- `main.sla`: Sla surrogate that preserves the same union payload read observable without an explicit `repr(C)` layout contract.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/113_union_ffi_types/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/113_union_ffi_types/main.sla --out /tmp/113_union_ffi_types.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/113_union_ffi_types/main.sla
```
