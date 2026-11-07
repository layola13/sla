# 280 Build Ci Cd Integration

This slot now uses a real fixture-backed CI/CD reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves stage counts instead of checking CI config, workflow files, and generated pipeline output.

- `main.rs`: Rust reference that reads `build/ci.toml`, `ci/workflows/*.yml`, and `generated/ci/pipeline.sa`.
- `main.sla`: current surrogate that only preserves the CI/CD stage count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/280_build_ci_cd_integration/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/280_build_ci_cd_integration/main.sla --out /tmp/280_build_ci_cd_integration.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/280_build_ci_cd_integration/main.sla
```
