# 209 Pkg Build Dependencies

This slot now uses a real build-generated fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a build-dependency count instead of true build-script execution or generated-source lifecycle behavior.

- `main.rs`: Rust reference that checks `sa.pkg`, the generated artifact module, and the `src` import of the generated output.
- `main.sla`: current surrogate that only preserves a one-build-dependency count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/209_pkg_build_dependencies/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/209_pkg_build_dependencies/main.sla --out /tmp/209_pkg_build_dependencies.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/209_pkg_build_dependencies/main.sla
```
