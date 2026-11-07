# 149 Packed Repr

This directory matches the packed-repr catalog slot.

- `main.rs`: Rust reference for a packed-layout field observable.
- `main.sla`: Sla companion for a packed-layout field observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/149_packed_repr/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/149_packed_repr/main.sla --out /tmp/149_packed_repr.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/149_packed_repr/main.sla
```
