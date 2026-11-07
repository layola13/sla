# 162 Auto Traits Send Sync

This directory keeps the auto-traits `Send`/`Sync` slot as an explicit surrogate.

- `main.rs`: Rust reference for `require_send<T: Send>(...)` over `Data`.
- `main.sla`: Sla surrogate for the accepted result observable.

Because the current Sla shape does not preserve the `require_send(d)` move path without a verifier failure, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/162_auto_traits_send_sync/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/162_auto_traits_send_sync/main.sla --out /tmp/162_auto_traits_send_sync.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/162_auto_traits_send_sync/main.sla
```
