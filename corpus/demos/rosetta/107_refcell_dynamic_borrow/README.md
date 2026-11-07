# 107 Refcell Dynamic Borrow

This directory matches the `RefCell` dynamic-borrow catalog slot.

- `main.rs`: Rust reference for borrow, scoped release, mutable borrow, and later readback.
- `main.sla`: Sla companion for borrow, scoped release, mutable borrow, and later readback.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/107_refcell_dynamic_borrow/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/107_refcell_dynamic_borrow/main.sla --out /tmp/107_refcell_dynamic_borrow.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/107_refcell_dynamic_borrow/main.sla
```
