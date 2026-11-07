# 309 Try Block Macro

> **Status**: Rust and `main.sa` carry the try-block fixture. Current Sla is a `❌` companion for `try { ... }` blocks, although it does exercise the existing `Result` and `?` path.

- `main.rs`: Rust reference using nightly-style `try { let a = parse_num("10")?; ... }` block semantics.
- `main.sa`: upstream SA-ASM fixture using result unwrap macros.
- `main.sla`: executable companion using current `Result<i32, i32>` and `?` support, but without native `try {}` block syntax.

## Verified

```bash
zig build local-cli -- sla test demos/rosetta/309_try_block_macro/main.sla
```
