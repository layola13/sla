# 154 Box From Raw

This directory matches the `Box::from_raw` catalog slot.

- `main.rs`: Rust reference for reconstructing box ownership from a raw pointer.
- `main.sla`: Sla companion for reconstructing box ownership from a raw pointer.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/154_box_from_raw/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/154_box_from_raw/main.sla --out /tmp/154_box_from_raw.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/154_box_from_raw/main.sla
```
