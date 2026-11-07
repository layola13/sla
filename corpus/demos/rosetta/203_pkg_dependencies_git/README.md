# 203 Pkg Dependencies Git

This slot now uses a real git-dependency reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count observable instead of true git package-resolution behavior.

- `main.rs`: Rust reference that reads `sa.pkg` plus `vendor/git_dep.sa` and checks the vendored git-dependency shape.
- `main.sla`: current surrogate that only preserves a one-dependency count observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/203_pkg_dependencies_git/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/203_pkg_dependencies_git/main.sla --out /tmp/203_pkg_dependencies_git.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/203_pkg_dependencies_git/main.sla
```
