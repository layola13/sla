# 110 Trait Super Vtable

This slot models a trait inheriting another trait and dispatching both methods.

- `main.rs`: Rust reference for a trait inheriting another trait and dispatching both methods.
- `main.sla`: Sla companion for the same dispatch path, with minimal call-result staging that is currently required for the checked addition path to type-check locally.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/110_trait_super_vtable/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/110_trait_super_vtable/main.sla --out /tmp/110_trait_super_vtable.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/110_trait_super_vtable/main.sla
```
