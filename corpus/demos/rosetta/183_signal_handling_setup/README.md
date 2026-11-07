# 183 Signal Handling Setup

Minimal signal-setup slot that returns the configured signal number `2` as its observable result.

- `main.rs`: Rust reference for the fixed signal constant.
- `main.sla`: Sla companion for the fixed signal constant.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/183_signal_handling_setup/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/183_signal_handling_setup/main.sla --out /tmp/183_signal_handling_setup.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/183_signal_handling_setup/main.sla
```
