# 292 Eco Wasm Memory Export

This slot now uses a real fixture-backed Wasm memory export integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `guest/memory_export.*`, memory layout JSON, host note, and memory map.
- `main.sla`: current surrogate that only preserves the memory-surface count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/292_eco_wasm_memory_export/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/292_eco_wasm_memory_export/main.sla --out /tmp/292_eco_wasm_memory_export.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/292_eco_wasm_memory_export/main.sla
```
