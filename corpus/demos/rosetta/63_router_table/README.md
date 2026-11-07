# 063 Router Table

This directory matches the router-table lookup topic for the catalog slot.

- `main.rs`: Rust reference for the route-to-handler map semantics used by this slot.
- `main.sla`: Sla companion for the route-to-handler map semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/63_router_table/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/63_router_table/main.sla --out /tmp/63_router_table.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/63_router_table/main.sla
```
