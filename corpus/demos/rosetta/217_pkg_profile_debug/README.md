# 217 Pkg Profile Debug

This slot now uses a real debug-profile fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a debug-profile count instead of true profile resolution.

- `main.rs`: Rust reference that reads the debug profile tree, helper constants, and package metadata.
- `main.sla`: current surrogate that only preserves a one-debug-profile count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/217_pkg_profile_debug/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/217_pkg_profile_debug/main.sla --out /tmp/217_pkg_profile_debug.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/217_pkg_profile_debug/main.sla
```
