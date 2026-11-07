# 143 Never Type Diverge

This directory keeps the never-type divergence slot as an explicit surrogate.

- `main.rs`: Rust reference for a real `fn fail() -> !` divergence path that is intentionally not taken.
- `main.sla`: Sla surrogate for the safe-path observable.

Because the current Sla parser does not accept the `-> !` shape used by the Rust source, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/143_never_type_diverge/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/143_never_type_diverge/main.sla --out /tmp/143_never_type_diverge.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/143_never_type_diverge/main.sla
```
