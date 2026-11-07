# 271 Build Optimization Passes

This slot now uses a real fixture-backed optimizer-pass reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves pass counts instead of checking pass config, cached order, and generated schedule output.

- `main.rs`: Rust reference that reads `build/optimizations/passes.toml`, `cache/optimizations/order.txt`, and `generated/optimizations/passes.sa`.
- `main.sla`: current surrogate that only preserves the optimization-pass count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/271_build_optimization_passes/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/271_build_optimization_passes/main.sla --out /tmp/271_build_optimization_passes.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/271_build_optimization_passes/main.sla
```
