# 128 Rcu Read Copy Update

This directory now records the current RCU surrogate honestly.

- `main.rs`: local Rust surrogate that keeps an old `Arc` snapshot alive while constructing a new snapshot.
- `main.sla`: matching Sla surrogate for the same old-snapshot plus new-snapshot observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/128_rcu_read_copy_update/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/128_rcu_read_copy_update/main.sla --out /tmp/128_rcu_read_copy_update.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/128_rcu_read_copy_update/main.sla
```
