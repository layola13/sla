# 294 Eco Os Kernel Module

This slot now uses a real fixture-backed OS kernel module integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `kernel/module.*`, module manifest, insmod notes, and linker script.
- `main.sla`: current surrogate that only preserves the kernel hook count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/294_eco_os_kernel_module/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/294_eco_os_kernel_module/main.sla --out /tmp/294_eco_os_kernel_module.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/294_eco_os_kernel_module/main.sla
```
