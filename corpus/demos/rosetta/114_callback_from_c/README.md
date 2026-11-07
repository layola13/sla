# 114 Callback From C

This directory matches the callback-from-C catalog slot.

- `main.rs`: Rust reference for passing an `extern "C"` function pointer into a caller.
- `main.sla`: Sla companion for passing an `extern "C"` function pointer into a caller.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/114_callback_from_c/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/114_callback_from_c/main.sla --out /tmp/114_callback_from_c.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/114_callback_from_c/main.sla
```
