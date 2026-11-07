# 218 Pkg Metadata Custom

This slot now uses a real custom-metadata fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a metadata-field count instead of true custom metadata handling.

- `main.rs`: Rust reference that reads the custom-key manifest field and nested metadata module tree.
- `main.sla`: current surrogate that only preserves a two-field metadata count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/218_pkg_metadata_custom/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/218_pkg_metadata_custom/main.sla --out /tmp/218_pkg_metadata_custom.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/218_pkg_metadata_custom/main.sla
```
