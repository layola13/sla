# 105 Let Else

This directory matches the `let else` catalog slot, unwrapping an `Option` through the early-exit pattern.

- `main.rs`: Rust reference for `let Some(x) = value else { ... }`.
- `main.sla`: Sla companion for `let Some(x) = value else { ... }`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/105_let_else/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/105_let_else/main.sla --out /tmp/105_let_else.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/105_let_else/main.sla
```
