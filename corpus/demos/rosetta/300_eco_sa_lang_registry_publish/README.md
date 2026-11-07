# 300 Eco Sa Lang Registry Publish

This slot now uses a real fixture-backed SA language registry publishing reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `registry/publish.*`, publish docs, registry JSON, and publish log.
- `main.sla`: current surrogate that only preserves the publish-step count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/300_eco_sa_lang_registry_publish/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/300_eco_sa_lang_registry_publish/main.sla --out /tmp/300_eco_sa_lang_registry_publish.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/300_eco_sa_lang_registry_publish/main.sla
```
