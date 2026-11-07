# 208 Pkg Dev Dependencies

This slot now uses a real dev-dependency fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a one-dependency observable instead of true dev-only package resolution.

- `main.rs`: Rust reference that checks the `dev/` helper/test path is declared separately from the release `src/` path.
- `main.sla`: current surrogate that only preserves a one-dev-dependency count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/208_pkg_dev_dependencies/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/208_pkg_dev_dependencies/main.sla --out /tmp/208_pkg_dev_dependencies.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/208_pkg_dev_dependencies/main.sla
```
