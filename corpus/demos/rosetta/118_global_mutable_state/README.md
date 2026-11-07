# 118 Global Mutable State

This directory matches the global-mutable-state catalog slot through the final counter observable.

- `main.rs`: Rust reference for two unsafe global counter updates.
- `main.sla`: Sla companion for two unsafe global counter updates.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/118_global_mutable_state/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/118_global_mutable_state/main.sla --out /tmp/118_global_mutable_state.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/118_global_mutable_state/main.sla
```
