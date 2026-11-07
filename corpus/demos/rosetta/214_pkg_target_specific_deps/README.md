# 214 Pkg Target Specific Deps

This slot now uses a real target-specific fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a target-branch count instead of true target-specific dependency resolution.

- `main.rs`: Rust reference that reads the target selector, native helper, portable helper, and dispatch module.
- `main.sla`: current surrogate that only preserves a two-branch dependency count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/214_pkg_target_specific_deps/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/214_pkg_target_specific_deps/main.sla --out /tmp/214_pkg_target_specific_deps.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/214_pkg_target_specific_deps/main.sla
```
