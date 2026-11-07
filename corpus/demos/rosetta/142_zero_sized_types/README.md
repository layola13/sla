# 142 Zero Sized Types

This directory matches the zero-sized-types catalog slot.

- `main.rs`: Rust reference for a zero-sized processing observable.
- `main.sla`: Sla companion for a zero-sized processing observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/142_zero_sized_types/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/142_zero_sized_types/main.sla --out /tmp/142_zero_sized_types.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/142_zero_sized_types/main.sla
```
