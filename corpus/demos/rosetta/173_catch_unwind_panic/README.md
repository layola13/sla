# 173 Catch Unwind Panic

This directory matches the catch-unwind/panic catalog slot.

- `main.rs`: Rust reference for converting a panic path into an observable result.
- `main.sla`: Sla companion for converting a panic path into an observable result.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/173_catch_unwind_panic/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/173_catch_unwind_panic/main.sla --out /tmp/173_catch_unwind_panic.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/173_catch_unwind_panic/main.sla
```
