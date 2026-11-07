# 109 Atomic Fetch Add

This directory matches the atomic fetch-add catalog slot.

- `main.rs`: Rust reference for old-value and new-value observation around `fetch_add`.
- `main.sla`: Sla companion for old-value and new-value observation around `fetch_add`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/109_atomic_fetch_add/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/109_atomic_fetch_add/main.sla --out /tmp/109_atomic_fetch_add.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/109_atomic_fetch_add/main.sla
```
