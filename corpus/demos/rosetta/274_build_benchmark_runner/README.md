# 274 Build Benchmark Runner

This slot now uses a real fixture-backed benchmark-runner reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves benchmark counts instead of checking bench manifest, case files, and generated runner output.

- `main.rs`: Rust reference that reads `bench/manifest.toml`, `bench/cases/*.toml`, and `generated/bench/runner.sa`.
- `main.sla`: current surrogate that only preserves the benchmark-group count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/274_build_benchmark_runner/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/274_build_benchmark_runner/main.sla --out /tmp/274_build_benchmark_runner.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/274_build_benchmark_runner/main.sla
```
