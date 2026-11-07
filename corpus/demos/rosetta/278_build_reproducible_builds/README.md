# 278 Build Reproducible Builds

This slot now uses a real fixture-backed reproducible-build reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one deterministic-output count instead of checking fixed seed, fingerprint, and generated artifact metadata.

- `main.rs`: Rust reference that reads `build/repro/seed.toml`, `cache/repro/fingerprint.txt`, and `generated/repro/build.sa`.
- `main.sla`: current surrogate that only preserves the deterministic-output count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/278_build_reproducible_builds/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/278_build_reproducible_builds/main.sla --out /tmp/278_build_reproducible_builds.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/278_build_reproducible_builds/main.sla
```
