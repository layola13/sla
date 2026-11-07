# 307 At Binding Macro

> **Status**: Rust and `main.sa` carry the real `n @ range` fixture. Current Sla is a `❌` companion, not 1:1 at-binding pattern support.

- `main.rs`: Rust reference using `n @ 1..=5` and `n @ 6..=10`.
- `main.sa`: upstream SA-ASM fixture using `BIND_RANGE` to bind and range-check a value.
- `main.sla`: executable companion that preserves the checked observable with explicit range checks and direct use of `x` while native at-binding lowering is absent.

## Verified

```bash
zig build local-cli -- sla test demos/rosetta/307_at_binding_macro/main.sla
```
