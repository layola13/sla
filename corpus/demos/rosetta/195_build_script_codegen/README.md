# 195 Build Script Codegen

This slot keeps build-script code generation observable as a generated-value surrogate.

- `main.rs`: Rust reference for a real generated include via `include!(concat!(env!("OUT_DIR"), "/generated.rs"))`.
- `main.sla`: Sla surrogate for consuming a generated constant value.

Because the Sla side does not execute a real build-script include path, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/195_build_script_codegen/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/195_build_script_codegen/main.sla --out /tmp/195_build_script_codegen.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/195_build_script_codegen/main.sla
```
