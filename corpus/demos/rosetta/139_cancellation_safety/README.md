# 139 Cancellation Safety

This directory matches the cancellation-safety catalog slot.

- `main.rs`: Rust reference for cancellation checks and recovery behavior.
- `main.sla`: Sla companion for cancellation checks and recovery behavior.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/139_cancellation_safety/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/139_cancellation_safety/main.sla --out /tmp/139_cancellation_safety.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/139_cancellation_safety/main.sla
```
