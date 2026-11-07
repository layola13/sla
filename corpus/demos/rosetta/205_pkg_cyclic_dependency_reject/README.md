# 205 Pkg Cyclic Dependency Reject

This slot now uses a real cyclic-import reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a cycle-count observable instead of performing true package-graph rejection.

- `main.rs`: Rust reference that reads `pkg_a/main.sa` and `pkg_b/main.sa` and checks the explicit cycle shape.
- `main.sla`: current surrogate that only preserves a one-cycle count observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/205_pkg_cyclic_dependency_reject/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/205_pkg_cyclic_dependency_reject/main.sla --out /tmp/205_pkg_cyclic_dependency_reject.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/205_pkg_cyclic_dependency_reject/main.sla
```
