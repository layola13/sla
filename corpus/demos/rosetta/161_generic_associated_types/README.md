# 161 Generic Associated Types

This directory matches the generic-associated-types catalog slot.

- `main.rs`: Rust reference for an associated borrowed-item observable.
- `main.sla`: Sla companion for an associated borrowed-item observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/161_generic_associated_types/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/161_generic_associated_types/main.sla --out /tmp/161_generic_associated_types.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/161_generic_associated_types/main.sla
```
