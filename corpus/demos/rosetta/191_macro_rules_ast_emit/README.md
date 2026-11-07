# 191 Macro Rules Ast Emit

This slot keeps the `macro_rules!` theme as an explicit surrogate.

- `main.rs`: Rust reference for `emit_sum!(left, right)` emitted through a real declarative macro.
- `main.sla`: Sla surrogate that preserves the emitted-sum observable through a plain helper function.

Because the Sla side does not execute a real `macro_rules!` expansion, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/191_macro_rules_ast_emit/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/191_macro_rules_ast_emit/main.sla --out /tmp/191_macro_rules_ast_emit.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/191_macro_rules_ast_emit/main.sla
```
