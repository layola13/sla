# 165 Blanket Impl Resolution

This directory keeps the blanket-impl-resolution slot as an explicit surrogate.

- `main.rs`: Rust reference for `trait Len` implemented over `[i32; 2]` and resolved via `arr.len()`.
- `main.sla`: Sla surrogate that preserves the array-length observable through the built-in `.len()` path.

The parser crash on array-target `impl` declarations is fixed locally now, but the full trait-resolution shape still does not type-check, so this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/165_blanket_impl_resolution/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/165_blanket_impl_resolution/main.sla --out /tmp/165_blanket_impl_resolution.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/165_blanket_impl_resolution/main.sla
```
