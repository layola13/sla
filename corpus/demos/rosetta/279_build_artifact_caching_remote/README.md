# 279 Build Artifact Caching Remote

This slot now uses a real fixture-backed remote-cache reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one downloaded-artifact count instead of checking remote cache index, state, and generated cache metadata.

- `main.rs`: Rust reference that reads `cache/remote/index.json`, `cache/remote/state.txt`, and `generated/remote/cache.sa`.
- `main.sla`: current surrogate that only preserves the remote-cache artifact count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/279_build_artifact_caching_remote/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/279_build_artifact_caching_remote/main.sla --out /tmp/279_build_artifact_caching_remote.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/279_build_artifact_caching_remote/main.sla
```
