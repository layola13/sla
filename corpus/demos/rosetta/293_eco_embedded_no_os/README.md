# 293 Eco Embedded No Os

This slot now uses a real fixture-backed bare-metal no-OS integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `guest/startup.*`, board docs, linker script, and memory map.
- `main.sla`: current surrogate that only preserves the reset-handler count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/293_eco_embedded_no_os/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/293_eco_embedded_no_os/main.sla --out /tmp/293_eco_embedded_no_os.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/293_eco_embedded_no_os/main.sla
```
