# 106 Cell Interior Mut

This directory matches the `Cell` interior-update catalog slot.

- `main.rs`: Rust reference for `Cell::get` / `Cell::set` observable behavior.
- `main.sla`: Sla companion for `Cell::get` / `Cell::set` observable behavior.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/106_cell_interior_mut/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/106_cell_interior_mut/main.sla --out /tmp/106_cell_interior_mut.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/106_cell_interior_mut/main.sla
```
