# 129 Seqlock Optimistic

This directory matches the seqlock optimistic-read catalog slot.

- `main.rs`: Rust reference for stable reads across a versioned update.
- `main.sla`: Sla companion for stable reads across a versioned update.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/129_seqlock_optimistic/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/129_seqlock_optimistic/main.sla --out /tmp/129_seqlock_optimistic.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/129_seqlock_optimistic/main.sla
```
