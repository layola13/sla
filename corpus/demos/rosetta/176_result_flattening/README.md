# 176 Result Flattening

This directory matches the result-flattening catalog slot.

- `main.rs`: Rust reference for flattening nested `Result` values.
- `main.sla`: Sla companion for flattening nested `Result` values.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/176_result_flattening/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/176_result_flattening/main.sla --out /tmp/176_result_flattening.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/176_result_flattening/main.sla
```
