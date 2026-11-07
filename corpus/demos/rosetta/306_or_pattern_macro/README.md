# 306 Or Pattern Macro

> **Status**: Rust and `main.sa` carry the real or-pattern fixture. Current Sla is a `❌` companion, not 1:1 `Red | Green | Blue` pattern syntax support.

- `main.rs`: Rust reference using `matches!(c, Color::Red | Color::Green | Color::Blue)`.
- `main.sa`: upstream SA-ASM fixture using `OR_TAG_3` over enum tags.
- `main.sla`: executable companion using enum `match` arms for each primary color. It intentionally does not claim native or-pattern support.

## Verified

```bash
zig build local-cli -- sla test demos/rosetta/306_or_pattern_macro/main.sla
```
