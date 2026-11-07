# 077 Http Route

This directory matches the route-status topic for the catalog slot.

- `main.rs`: Rust reference for the route semantics used by this slot.
- `main.sla`: Sla companion for the route semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/77_http_route/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/77_http_route/main.sla --out /tmp/77_http_route.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/77_http_route/main.sla
```
