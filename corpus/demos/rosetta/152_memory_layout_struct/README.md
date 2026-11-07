# 152 Memory Layout Struct

This directory matches the memory-layout-struct catalog slot.

- `main.rs`: Rust reference for a struct layout observable.
- `main.sla`: Sla companion for a struct layout observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/152_memory_layout_struct/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/152_memory_layout_struct/main.sla --out /tmp/152_memory_layout_struct.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/152_memory_layout_struct/main.sla
```
