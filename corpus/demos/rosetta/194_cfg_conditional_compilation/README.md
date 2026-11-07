# 194 Cfg Conditional Compilation

This slot keeps conditional-compilation shape observable as a selected target-architecture string length surrogate.

- `main.rs`: Rust reference for the real `#[cfg(...)]` target-arch branch selection.
- `main.sla`: Sla surrogate for the selected target-architecture string observable.

Because the Sla side does not perform real conditional compilation, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/194_cfg_conditional_compilation/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/194_cfg_conditional_compilation/main.sla --out /tmp/194_cfg_conditional_compilation.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/194_cfg_conditional_compilation/main.sla
```
