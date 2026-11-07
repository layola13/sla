# 150 C Repr Alignment

This directory matches the C-repr alignment catalog slot.

- `main.rs`: Rust reference for an alignment/layout observable.
- `main.sla`: Sla companion for an alignment/layout observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/150_c_repr_alignment/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/150_c_repr_alignment/main.sla --out /tmp/150_c_repr_alignment.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/150_c_repr_alignment/main.sla
```
