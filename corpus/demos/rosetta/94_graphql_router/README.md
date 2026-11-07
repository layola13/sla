# 094 Graphql Router

This directory matches the GraphQL routing topic for the catalog slot, selecting a resolver from operation, field, and nesting state.

- `main.rs`: Rust reference for the operation/field resolver selection semantics used by this slot.
- `main.sla`: Sla companion for the operation/field resolver selection semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/94_graphql_router/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/94_graphql_router/main.sla --out /tmp/94_graphql_router.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/94_graphql_router/main.sla
```
