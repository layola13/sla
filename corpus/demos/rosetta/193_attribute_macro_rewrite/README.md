# 193 Attribute Macro Rewrite

This slot keeps the attribute-rewrite theme as an explicit surrogate.

- `main.rs`: Rust reference for a real `#[rewrite]` attribute on `rewrite_value(&mut value)`.
- `main.sla`: Sla surrogate that preserves the value-flow observable without claiming real attribute-macro rewriting.

Because the Sla side does not execute a real attribute macro rewrite, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/193_attribute_macro_rewrite/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/193_attribute_macro_rewrite/main.sla --out /tmp/193_attribute_macro_rewrite.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/193_attribute_macro_rewrite/main.sla
```
