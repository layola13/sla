# 093 Log Aggregator

This directory matches the log-severity aggregation topic for the catalog slot, combining weighted severities with dropped-line accounting.

- `main.rs`: Rust reference for the weighted severity-score semantics used by this slot.
- `main.sla`: Sla companion for the weighted severity-score semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/93_log_aggregator/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/93_log_aggregator/main.sla --out /tmp/93_log_aggregator.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/93_log_aggregator/main.sla
```
