# 086 Cache Eviction

This slot models cache eviction by selecting the least-recently-used entry from a small set.

- `main.rs`: Rust reference for the eviction choice.
- `main.sla`: Sla companion for the eviction choice.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/86_cache_eviction/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/86_cache_eviction/main.sla --out /tmp/86_cache_eviction.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/86_cache_eviction/main.sla
```
