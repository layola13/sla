# 273 Build Test Harness

This slot now uses a real fixture-backed test-harness reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves case counts instead of checking harness manifest, case files, and generated index output.

- `main.rs`: Rust reference that reads `harness/manifest.toml`, `harness/cases/*.toml`, and `generated/harness/index.sa`.
- `main.sla`: current surrogate that only preserves the harness-case count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/273_build_test_harness/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/273_build_test_harness/main.sla --out /tmp/273_build_test_harness.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/273_build_test_harness/main.sla
```
