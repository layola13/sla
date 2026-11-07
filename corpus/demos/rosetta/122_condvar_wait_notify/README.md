# 122 Condvar Wait Notify

This directory matches the condvar wait-notify catalog slot.

- `main.rs`: Rust reference for waiting on a mutex-backed ready flag and waking through `notify_one()`.
- `main.sla`: Sla companion for waiting on a mutex-backed ready flag and waking through `notify_one()`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/122_condvar_wait_notify/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/122_condvar_wait_notify/main.sla --out /tmp/122_condvar_wait_notify.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/122_condvar_wait_notify/main.sla
```
