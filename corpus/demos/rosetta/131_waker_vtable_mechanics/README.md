# 131 Waker Vtable Mechanics

This directory matches the waker vtable mechanics catalog slot.

- `main.rs`: Rust reference for a custom `RawWakerVTable` whose clone, wake, and wake-by-ref callbacks update a shared atomic wake count.
- `main.sla`: Sla companion for a custom `RawWakerVTable` whose clone, wake, and wake-by-ref callbacks update a shared atomic wake count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/131_waker_vtable_mechanics/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/131_waker_vtable_mechanics/main.sla --out /tmp/131_waker_vtable_mechanics.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/131_waker_vtable_mechanics/main.sla
```
