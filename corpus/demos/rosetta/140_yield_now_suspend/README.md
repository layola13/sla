# 140 Yield Now Suspend

This directory now records the current yield/suspend gap honestly.

- `main.rs`: Rust reference using a real `tokio::task::yield_now().await` suspension point.
- `main.sla`: Sla surrogate that keeps only the resumed-value observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/140_yield_now_suspend/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/140_yield_now_suspend/main.sla --out /tmp/140_yield_now_suspend.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/140_yield_now_suspend/main.sla
```
