# 296 Eco Gpu Ptx Shader

This slot now uses a real fixture-backed GPU PTX shader integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `guest/shader.*`, kernel args, launch config, and PTX note.
- `main.sla`: current surrogate that only preserves the compute-kernel count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/296_eco_gpu_ptx_shader/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/296_eco_gpu_ptx_shader/main.sla --out /tmp/296_eco_gpu_ptx_shader.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/296_eco_gpu_ptx_shader/main.sla
```
