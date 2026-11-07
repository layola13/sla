# 092 Query Plan

This directory matches the query-cost selection topic for the catalog slot.

- `main.rs`: Rust reference for the plan-cost comparison semantics used by this slot.
- `main.sla`: Sla companion for the plan-cost comparison semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/92_query_plan/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/92_query_plan/main.sla --out /tmp/92_query_plan.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/92_query_plan/main.sla
```
