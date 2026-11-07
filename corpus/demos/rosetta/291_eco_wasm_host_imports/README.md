# 291 Eco Wasm Host Imports

This slot now uses a real fixture-backed Wasm host-import integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `guest/guest_entry.*`, `host/host_imports.sai`, WIT notes, and runtime docs.
- `main.sla`: current surrogate that only preserves the host-import count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/291_eco_wasm_host_imports/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/291_eco_wasm_host_imports/main.sla --out /tmp/291_eco_wasm_host_imports.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/291_eco_wasm_host_imports/main.sla
```
