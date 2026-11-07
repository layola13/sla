# 305 Range Pattern Macro

> **Status**: Rust and `main.sa` carry the real range-pattern fixture. Current Sla is a `❌` companion, not 1:1 range-pattern syntax support.

- `main.rs`: Rust reference using `match score { 0..=59, 60..=79, ... }`.
- `main.sa`: upstream SA-ASM fixture using a `RANGE_IN` macro to model closed interval matching.
- `main.sla`: executable companion that checks the same closed intervals with explicit control flow so the observable values stay testable while native range-pattern lowering is absent.

## Verified

```bash
zig build local-cli -- sla test demos/rosetta/305_range_pattern_macro/main.sla
```
