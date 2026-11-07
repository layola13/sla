# 130 Park Unpark Thread

This directory matches the park/unpark thread catalog slot.

- `main.rs`: Rust reference for `thread::current()`, `thread::park()`, `unpark()`, and resumed worker completion.
- `main.sla`: Sla companion for `thread::current()`, `thread::park()`, `unpark()`, and resumed worker completion.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/130_park_unpark_thread/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/130_park_unpark_thread/main.sla --out /tmp/130_park_unpark_thread.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/130_park_unpark_thread/main.sla
```
