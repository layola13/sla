# 198 Control Flow Guard Cfi

This slot keeps control-flow-integrity checking observable as a guarded-call surrogate returning `2`.

- `main.rs`: Rust reference for the guarded function-pointer call.
- `main.sla`: Sla surrogate that calls the target directly.

Because the current Sla path does not preserve the local function-pointer binding shape, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/198_control_flow_guard_cfi/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/198_control_flow_guard_cfi/main.sla --out /tmp/198_control_flow_guard_cfi.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/198_control_flow_guard_cfi/main.sla
```
