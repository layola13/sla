# 178 Panic Hook Override

This directory keeps the panic-hook override slot as an explicit surrogate.

- `main.rs`: Rust reference for real `std::panic::set_hook(...)` installation followed by a panic.
- `main.sla`: Sla surrogate for the observable "hook would print before panic" path.

Because the Sla side does not override a real panic hook, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/178_panic_hook_override/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/178_panic_hook_override/main.sla --out /tmp/178_panic_hook_override.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/178_panic_hook_override/main.sla
```
