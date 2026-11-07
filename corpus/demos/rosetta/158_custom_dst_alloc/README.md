# 158 Custom Dst Alloc

This directory keeps the custom-DST allocation slot as an explicit surrogate.

- `main.rs`: Rust surrogate that owns bytes and observes their length.
- `main.sla`: Sla companion for the same owned-byte-length observable.

It is not a literal custom DST allocation demo, so this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/158_custom_dst_alloc/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/158_custom_dst_alloc/main.sla --out /tmp/158_custom_dst_alloc.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/158_custom_dst_alloc/main.sla
```
