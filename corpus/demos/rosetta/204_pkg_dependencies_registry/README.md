# 204 Pkg Dependencies Registry

This slot now uses a real registry-dependency reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count observable instead of true registry resolution behavior.

- `main.rs`: Rust reference that reads `sa.pkg` plus `registry/codec.sa` and checks the cached registry-dependency shape.
- `main.sla`: current surrogate that only preserves a one-dependency count observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/204_pkg_dependencies_registry/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/204_pkg_dependencies_registry/main.sla --out /tmp/204_pkg_dependencies_registry.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/204_pkg_dependencies_registry/main.sla
```
