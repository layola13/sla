# 207 Pkg Multiple Versions Conflict

This slot now uses a real multi-version conflict fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a conflict flag instead of performing true package-graph rejection.

- `main.rs`: Rust reference that reads the root manifest, resolver, two versioned package manifests, and duplicate public symbols.
- `main.sla`: current surrogate that only preserves a reported-conflict flag.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/207_pkg_multiple_versions_conflict/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/207_pkg_multiple_versions_conflict/main.sla --out /tmp/207_pkg_multiple_versions_conflict.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/207_pkg_multiple_versions_conflict/main.sla
```
