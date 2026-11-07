# 097 Sync Service

This directory matches the sync-decision topic for the catalog slot, combining dirty state, remote freshness, and offline blocking.

- `main.rs`: Rust reference for the dirty/version-based sync semantics used by this slot.
- `main.sla`: Sla companion for the dirty/version-based sync semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/97_sync_service/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/97_sync_service/main.sla --out /tmp/97_sync_service.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/97_sync_service/main.sla
```
