# 151 Global Alloc Trait

This directory matches the global-allocator trait catalog slot.

- `main.rs`: Rust reference for an allocator-themed observable.
- `main.sla`: Sla companion for an allocator-themed observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/151_global_alloc_trait/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/151_global_alloc_trait/main.sla --out /tmp/151_global_alloc_trait.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/151_global_alloc_trait/main.sla
```
