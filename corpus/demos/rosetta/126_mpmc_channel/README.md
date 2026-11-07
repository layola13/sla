# 126 Mpmc Channel

This directory now records the current channel-surrogate boundary honestly.

- `main.rs`: Rust reference for a multi-producer send path that drains four values through one receiver.
- `main.sla`: matching Sla surrogate for the same multi-producer single-receiver observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/126_mpmc_channel/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/126_mpmc_channel/main.sla --out /tmp/126_mpmc_channel.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/126_mpmc_channel/main.sla
```
