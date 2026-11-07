# 141 Dynamically Sized Types

This directory matches the dynamically-sized-types catalog slot.

- `main.rs`: Rust reference for a DST length observable.
- `main.sla`: Sla companion for a DST length observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/141_dynamically_sized_types/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/141_dynamically_sized_types/main.sla --out /tmp/141_dynamically_sized_types.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/141_dynamically_sized_types/main.sla
```
