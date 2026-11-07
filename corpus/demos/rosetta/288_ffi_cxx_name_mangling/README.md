# 288 Ffi Cxx Name Mangling

This slot now uses a real fixture-backed C++ name-mangling FFI linkage reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/cxx_gate.*`, `ffi/cxx_name.sai`, C++ header, linker map, and `nm` output.
- `main.sla`: current surrogate that only preserves the C++ symbol-name count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/288_ffi_cxx_name_mangling/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/288_ffi_cxx_name_mangling/main.sla --out /tmp/288_ffi_cxx_name_mangling.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/288_ffi_cxx_name_mangling/main.sla
```
