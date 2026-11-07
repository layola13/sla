# 210 Pkg Workspace Root

This slot now uses a real workspace-member fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a member-count observable instead of true workspace package resolution.

- `main.rs`: Rust reference that reads the workspace manifest plus both member manifests and checks the aggregator imports.
- `main.sla`: current surrogate that only preserves a two-member count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/210_pkg_workspace_root/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/210_pkg_workspace_root/main.sla --out /tmp/210_pkg_workspace_root.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/210_pkg_workspace_root/main.sla
```
