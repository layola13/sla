# 265 Build Custom Linker Script

This slot now uses a real fixture-backed linker-script reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves section counts instead of checking linker config, memory map, script, and generated link plan.

- `main.rs`: Rust reference that reads `build/linker.toml`, `linker/linker.ld`, `linker/memory.x`, and `generated/link_plan.sa`.
- `main.sla`: current surrogate that only preserves the linker-section count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/265_build_custom_linker_script/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/265_build_custom_linker_script/main.sla --out /tmp/265_build_custom_linker_script.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/265_build_custom_linker_script/main.sla
```
