# 147 Custom Dst Pointers

This directory keeps the custom-DST pointer slot as an explicit surrogate.

- `main.rs`: Rust surrogate for observing the byte length carried by a DST-like slice pointer.
- `main.sla`: Sla companion for the same byte-length observable.

It is not a literal custom DST pointer implementation, so this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/147_custom_dst_pointers/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/147_custom_dst_pointers/main.sla --out /tmp/147_custom_dst_pointers.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/147_custom_dst_pointers/main.sla
```
