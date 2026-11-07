# 192 Proc Macro Derive Ast

This slot keeps the derive-themed observable as an explicit surrogate.

- `main.rs`: Rust reference for `#[derive(Clone, Copy)]` over `Pair`, followed by `let copy = pair` and field access.
- `main.sla`: Sla surrogate that preserves the `Pair` field-sum observable without claiming support for derived `Clone`/`Copy` behavior.

Because the Sla side does not model proc-macro derive expansion or the copied-value shape here, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/192_proc_macro_derive_ast/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/192_proc_macro_derive_ast/main.sla --out /tmp/192_proc_macro_derive_ast.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/192_proc_macro_derive_ast/main.sla
```
