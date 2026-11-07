# 167 Const Generics Expansion

This directory now records the const-generics gap honestly.

- `main.rs`: Rust reference using a real `const N: usize` array-length helper.
- `main.sla`: Sla surrogate keeping only the fixed-array `.len()` observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/167_const_generics_expansion/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/167_const_generics_expansion/main.sla --out /tmp/167_const_generics_expansion.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/167_const_generics_expansion/main.sla
```
