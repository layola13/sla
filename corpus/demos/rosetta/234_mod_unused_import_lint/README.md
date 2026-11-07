# 234 Mod Unused Import Lint

This slot now uses a real lint fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a lint-count observable instead of true unused-import diagnostics.

- `main.rs`: Rust reference that reads the used and unused branches and their seed modules.
- `main.sla`: current surrogate that only preserves a one-lint count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/234_mod_unused_import_lint/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/234_mod_unused_import_lint/main.sla --out /tmp/234_mod_unused_import_lint.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/234_mod_unused_import_lint/main.sla
```
