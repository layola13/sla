# 202 Pkg Dependencies Local

This slot now uses a real local-dependency reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count observable instead of true package-resolution behavior.

- `main.rs`: Rust reference that reads `sa.pkg` plus `pkg/local_dep.sa` and checks the local path-dependency shape.
- `main.sla`: current surrogate that only preserves a one-dependency count observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/202_pkg_dependencies_local/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/202_pkg_dependencies_local/main.sla --out /tmp/202_pkg_dependencies_local.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/202_pkg_dependencies_local/main.sla
```
