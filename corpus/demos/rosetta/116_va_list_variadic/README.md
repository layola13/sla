# 116 Va List Variadic

This directory now records the current variadic surrogate honestly.

- `main.rs`: local Rust surrogate that sums a slice of values.
- `main.sla`: matching Sla surrogate for the same slice-sum observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/116_va_list_variadic/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/116_va_list_variadic/main.sla --out /tmp/116_va_list_variadic.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/116_va_list_variadic/main.sla
```
