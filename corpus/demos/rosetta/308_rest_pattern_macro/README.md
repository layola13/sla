# 308 Rest Pattern Macro

> **Status**: Rust and `main.sa` carry the real rest-pattern fixture. Current Sla is a `❌` companion, not 1:1 `[a, b, ..]` or `(head, ..)` support.

- `main.rs`: Rust reference using slice and tuple rest patterns.
- `main.sa`: upstream SA-ASM fixture using macros to load only the leading fields.
- `main.sla`: executable companion that keeps the same first-two/head observable with ordinary parameters and tuple destructuring accepted by the current Sla compiler.

## Verified

```bash
zig build local-cli -- sla test demos/rosetta/308_rest_pattern_macro/main.sla
```
