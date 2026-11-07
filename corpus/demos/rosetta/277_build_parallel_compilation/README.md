# 277 Build Parallel Compilation

This slot now uses a real fixture-backed parallel-compilation reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves codegen-unit counts instead of checking parallel job manifests and generated schedule output.

- `main.rs`: Rust reference that reads `build/parallel/jobs/*.toml` and `generated/parallel/schedule.sa`.
- `main.sla`: current surrogate that only preserves the parallel-unit count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/277_build_parallel_compilation/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/277_build_parallel_compilation/main.sla --out /tmp/277_build_parallel_compilation.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/277_build_parallel_compilation/main.sla
```
