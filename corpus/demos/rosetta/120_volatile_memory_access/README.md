# 120 Volatile Memory Access

This directory matches the volatile-memory-access catalog slot.

- `main.rs`: Rust reference for `std::ptr::read_volatile` on an integer pointer.
- `main.sla`: Sla companion that explicitly imports `sa_std/ptr.sa` and calls `ptr::read_volatile` through the SLA-side facade.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/120_volatile_memory_access/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/120_volatile_memory_access/main.sla --out /tmp/120_volatile_memory_access.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/120_volatile_memory_access/main.sla
```
