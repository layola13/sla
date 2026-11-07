# 145 Opaque Type Alias

This directory matches the opaque-type-alias catalog slot.

- `main.rs`: Rust reference for an opaque alias observable.
- `main.sla`: Sla companion for an opaque alias observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/145_opaque_type_alias/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/145_opaque_type_alias/main.sla --out /tmp/145_opaque_type_alias.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/145_opaque_type_alias/main.sla
```
