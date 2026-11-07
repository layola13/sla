# 269 Build Cross Compile Windows

This slot now uses a real fixture-backed Windows target reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one target-triple count instead of checking target config and generated profile output.

- `main.rs`: Rust reference that reads `build/windows/target.toml` and `generated/windows/profile.sa`.
- `main.sla`: current surrogate that only preserves the Windows target count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/269_build_cross_compile_windows/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/269_build_cross_compile_windows/main.sla --out /tmp/269_build_cross_compile_windows.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/269_build_cross_compile_windows/main.sla
```
