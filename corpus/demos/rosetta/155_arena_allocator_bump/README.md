# 155 Arena Allocator Bump

This directory matches the current local arena-allocator-bump slot shape.

- `main.rs`: Rust reference for the current local observable `a + b` over two arena-themed values.
- `main.sla`: Sla companion for the same current local observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/155_arena_allocator_bump/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/155_arena_allocator_bump/main.sla --out /tmp/155_arena_allocator_bump.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/155_arena_allocator_bump/main.sla
```
