# 169 Negative Impls

This directory keeps the negative-impls slot as an explicit surrogate.

- `main.rs`: Rust reference for a real `impl !Send for UnsafeData {}` negative impl.
- `main.sla`: Sla surrogate that preserves the `UnsafeData` carrier shape without claiming support for negative impls.

Because the Sla side does not model `impl !Send`, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/169_negative_impls/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/169_negative_impls/main.sla --out /tmp/169_negative_impls.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/169_negative_impls/main.sla
```
