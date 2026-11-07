# 123 Barrier Sync

This directory matches the barrier sync catalog slot.

- `main.rs`: Rust reference for three threads waiting on a shared barrier and each resuming with `1`.
- `main.sla`: Sla companion for three threads waiting on a shared barrier and each resuming with `1`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/123_barrier_sync/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/123_barrier_sync/main.sla --out /tmp/123_barrier_sync.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/123_barrier_sync/main.sla
```
