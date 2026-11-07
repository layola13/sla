# 079 Metrics

This directory matches the success-rate topic for the catalog slot.

- `main.rs`: Rust reference for the throughput-style metric used by this slot.
- `main.sla`: Sla companion for the throughput-style metric used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/79_metrics/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/79_metrics/main.sla --out /tmp/79_metrics.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/79_metrics/main.sla
```
