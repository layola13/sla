# 091 Db Session

This slot models a database session with connection and transaction state.

- `main.rs`: Rust reference for the commitability check.
- `main.sla`: Sla companion for the commitability check.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/91_db_session/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/91_db_session/main.sla --out /tmp/91_db_session.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/91_db_session/main.sla
```
