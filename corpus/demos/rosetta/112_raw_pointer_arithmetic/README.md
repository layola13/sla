# 112 Raw Pointer Arithmetic

This directory matches the raw-pointer arithmetic catalog slot.

- `main.rs`: Rust reference for pointer offset and unsafe dereference observation.
- `main.sla`: Sla companion for pointer offset and unsafe dereference observation.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/112_raw_pointer_arithmetic/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/112_raw_pointer_arithmetic/main.sla --out /tmp/112_raw_pointer_arithmetic.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/112_raw_pointer_arithmetic/main.sla
```
