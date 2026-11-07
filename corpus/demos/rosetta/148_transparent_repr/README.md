# 148 Transparent Repr

This directory matches the transparent-repr catalog slot.

- `main.rs`: Rust reference for a transparent representation observable.
- `main.sla`: Sla companion for a transparent representation observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/148_transparent_repr/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/148_transparent_repr/main.sla --out /tmp/148_transparent_repr.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/148_transparent_repr/main.sla
```
