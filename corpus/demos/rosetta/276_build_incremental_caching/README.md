# 276 Build Incremental Caching

This slot now uses a real fixture-backed incremental-cache reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves cache-hit counts instead of checking cache strategy, index, hashes, and generated cache state.

- `main.rs`: Rust reference that reads `build/cache.toml`, `cache/index.json`, `cache/hashes.txt`, and `generated/cache/state.sa`.
- `main.sla`: current surrogate that only preserves the unchanged-module count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/276_build_incremental_caching/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/276_build_incremental_caching/main.sla --out /tmp/276_build_incremental_caching.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/276_build_incremental_caching/main.sla
```
