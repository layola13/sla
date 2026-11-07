# 211 Pkg Workspace Inheritance

This slot now uses a real workspace-inheritance fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves inherited-field counts instead of true workspace metadata inheritance.

- `main.rs`: Rust reference that reads the workspace manifest, shared config, shared package, and both inheriting member packages.
- `main.sla`: current surrogate that only preserves a two-field inheritance count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/211_pkg_workspace_inheritance/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/211_pkg_workspace_inheritance/main.sla --out /tmp/211_pkg_workspace_inheritance.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/211_pkg_workspace_inheritance/main.sla
```
