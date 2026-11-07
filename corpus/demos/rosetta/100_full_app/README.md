# 100 Full App

This directory matches the full-request handling topic for the catalog slot, combining authentication, routing, database, and rate-limit status.

- `main.rs`: Rust reference for the authenticated/route/db status semantics used by this slot.
- `main.sla`: Sla companion for the authenticated/route/db status semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/100_full_app/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/100_full_app/main.sla --out /tmp/100_full_app.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/100_full_app/main.sla
```
