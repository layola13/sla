# 160 Manually Drop Union

This directory matches the `ManuallyDrop` union catalog slot.

- `main.rs`: Rust reference for storing and extracting a union payload without automatic drop.
- `main.sla`: Sla companion for storing and extracting a union payload without automatic drop.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/160_manually_drop_union/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/160_manually_drop_union/main.sla --out /tmp/160_manually_drop_union.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/160_manually_drop_union/main.sla
```
