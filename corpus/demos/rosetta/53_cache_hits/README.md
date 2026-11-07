# 053 Cache Hits

This directory matches the cache-hit lookup topic for the catalog slot.

- `main.rs`: Rust reference for the HashMap cache-hit semantics used by this slot.
- `main.sla`: Sla companion for the HashMap cache-hit semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/53_cache_hits/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/53_cache_hits/main.sla --out /tmp/53_cache_hits.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/53_cache_hits/main.sla
```
