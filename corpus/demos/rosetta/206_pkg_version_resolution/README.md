# 206 Pkg Version Resolution

This slot now uses a real versioned-package fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves the selected patch observable instead of true package version resolution.

- `main.rs`: Rust reference that reads the root `sa.pkg`, both version manifests, and the resolver module before selecting the newer version.
- `main.sla`: current surrogate that only preserves a selected patch value.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/206_pkg_version_resolution/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/206_pkg_version_resolution/main.sla --out /tmp/206_pkg_version_resolution.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/206_pkg_version_resolution/main.sla
```
