# 268 Build Cross Compile Wasm

This slot now uses a real fixture-backed Wasm target reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one target-triple count instead of checking target config and generated profile output.

- `main.rs`: Rust reference that reads `build/wasm/target.toml` and `generated/wasm/profile.sa`.
- `main.sla`: current surrogate that only preserves the Wasm target count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/268_build_cross_compile_wasm/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/268_build_cross_compile_wasm/main.sla --out /tmp/268_build_cross_compile_wasm.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/268_build_cross_compile_wasm/main.sla
```
