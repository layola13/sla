# 171 Anyhow Dynamic Error

This directory now records the current dynamic-error gap honestly.

- `main.rs`: Rust reference with a boxed dynamic-error path before `map(...).unwrap_or(0)`.
- `main.sla`: Sla surrogate using a plain `Result<i32, i32>` error path for the same fallback observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/171_anyhow_dynamic_error/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/171_anyhow_dynamic_error/main.sla --out /tmp/171_anyhow_dynamic_error.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/171_anyhow_dynamic_error/main.sla
```
