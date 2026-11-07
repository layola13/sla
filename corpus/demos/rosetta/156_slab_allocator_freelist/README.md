# 156 Slab Allocator Freelist

This directory matches the current local slab-allocator-freelist slot shape.

- `main.rs`: Rust reference for the current local observable `node1 + node2`.
- `main.sla`: Sla companion for the same current local observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/156_slab_allocator_freelist/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/156_slab_allocator_freelist/main.sla --out /tmp/156_slab_allocator_freelist.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/156_slab_allocator_freelist/main.sla
```
