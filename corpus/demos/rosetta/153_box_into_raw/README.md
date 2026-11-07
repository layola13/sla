# 153 Box Into Raw

This directory matches the `Box::into_raw` catalog slot.

- `main.rs`: Rust reference for transferring box ownership into a raw pointer.
- `main.sla`: Sla companion for transferring box ownership into a raw pointer.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/153_box_into_raw/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/153_box_into_raw/main.sla --out /tmp/153_box_into_raw.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/153_box_into_raw/main.sla
```
