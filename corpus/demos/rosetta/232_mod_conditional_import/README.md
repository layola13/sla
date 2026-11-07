# 232 Mod Conditional Import

This slot now uses a real profile-branch fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a branch-count observable instead of true conditional module selection.

- `main.rs`: Rust reference that reads the selector plus both native and portable profile trees.
- `main.sla`: current surrogate that only preserves a one-branch count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/232_mod_conditional_import/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/232_mod_conditional_import/main.sla --out /tmp/232_mod_conditional_import.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/232_mod_conditional_import/main.sla
```
