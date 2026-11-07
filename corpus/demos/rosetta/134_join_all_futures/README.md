# 134 Join All Futures

This directory now records the current join-all surrogate honestly.

- `main.rs`: local Rust surrogate that awaits three futures in sequence and sums them.
- `main.sla`: matching Sla surrogate for the same sequential-await observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/134_join_all_futures/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/134_join_all_futures/main.sla --out /tmp/134_join_all_futures.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/134_join_all_futures/main.sla
```
