# 179 Assert Macro Expansion

This directory keeps the assert-macro expansion slot as an explicit surrogate.

- `main.rs`: Rust reference for a real `assert!(1 + 1 == 2)` expansion.
- `main.sla`: Sla surrogate that preserves the assertion-success observable through an explicit conditional panic.

Because the current Sla path does not support the `assert!` macro surface directly here, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/179_assert_macro_expansion/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/179_assert_macro_expansion/main.sla --out /tmp/179_assert_macro_expansion.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/179_assert_macro_expansion/main.sla
```
