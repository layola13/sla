# 220 Pkg Lib Dynamic

This slot now uses a real host/library fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a static-plus-dynamic artifact count instead of true library packaging.

- `main.rs`: Rust reference that reads the host tree, the library tree, and the exported interface.
- `main.sla`: current surrogate that only preserves a two-artifact count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/220_pkg_lib_dynamic/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/220_pkg_lib_dynamic/main.sla --out /tmp/220_pkg_lib_dynamic.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/220_pkg_lib_dynamic/main.sla
```
