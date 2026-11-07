# 275 Build Doc Generator

This slot now uses a real fixture-backed doc-generator reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves page counts instead of checking docgen config, source docs, and generated documentation index.

- `main.rs`: Rust reference that reads `build/docgen.toml`, `docs/spec.md`, `docs/api/index.md`, and `generated/docs/index.sa`.
- `main.sla`: current surrogate that only preserves the generated-page count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/275_build_doc_generator/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/275_build_doc_generator/main.sla --out /tmp/275_build_doc_generator.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/275_build_doc_generator/main.sla
```
