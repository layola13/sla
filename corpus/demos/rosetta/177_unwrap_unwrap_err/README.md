# 177 Unwrap Unwrap Err

This directory now records the `unwrap_err` gap honestly.

- `main.rs`: Rust reference using both `unwrap()` and `unwrap_err()`.
- `main.sla`: Sla companion that keeps the same final observable, but models the `unwrap_err()` half with an explicit `match Err(...)` surrogate.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/177_unwrap_unwrap_err/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/177_unwrap_unwrap_err/main.sla --out /tmp/177_unwrap_unwrap_err.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/177_unwrap_unwrap_err/main.sla
```
