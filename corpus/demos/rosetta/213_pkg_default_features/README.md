# 213 Pkg Default Features

This slot now uses a real default-feature fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a default-feature count instead of true default feature activation.

- `main.rs`: Rust reference that reads the package manifest, nested `src/defaults` modules, and the default feature constant.
- `main.sla`: current surrogate that only preserves a one-default-feature count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/213_pkg_default_features/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/213_pkg_default_features/main.sla --out /tmp/213_pkg_default_features.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/213_pkg_default_features/main.sla
```
