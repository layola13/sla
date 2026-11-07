# 212 Pkg Feature Flags

This slot now uses a real nested feature-flag fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves an enabled-feature count instead of true feature resolution.

- `main.rs`: Rust reference that reads the package manifest, nested `src/flags` modules, and all feature constants.
- `main.sla`: current surrogate that only preserves a two-feature count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/212_pkg_feature_flags/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/212_pkg_feature_flags/main.sla --out /tmp/212_pkg_feature_flags.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/212_pkg_feature_flags/main.sla
```
