# 235 Mod Transitive Dependency

This slot now uses a real transitive-dependency fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a transitive-count observable instead of true reachability analysis.

- `main.rs`: Rust reference that reads the `dep/` chain and the legacy flat path files.
- `main.sla`: current surrogate that only preserves a one-leaf count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/235_mod_transitive_dependency/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/235_mod_transitive_dependency/main.sla --out /tmp/235_mod_transitive_dependency.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/235_mod_transitive_dependency/main.sla
```
