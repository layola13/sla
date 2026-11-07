# 262 Build Bindgen C Header

This slot now uses a real fixture-backed bindgen reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves declaration counts instead of checking the bindgen config, C headers, and generated binding output.

- `main.rs`: Rust reference that reads `bindgen/bindgen.toml`, `bindgen/include/*.h`, and `generated/bindings.sa`.
- `main.sla`: current surrogate that only preserves the type/function declaration count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/262_build_bindgen_c_header/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/262_build_bindgen_c_header/main.sla --out /tmp/262_build_bindgen_c_header.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/262_build_bindgen_c_header/main.sla
```
