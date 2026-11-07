# 108 Atomic Spin Lock

This directory matches the atomic spin-lock catalog slot.

- `main.rs`: Rust reference for compare-exchange acquisition and release-store behavior.
- `main.sla`: Sla companion for compare-exchange acquisition and release-store behavior.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/108_atomic_spin_lock/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/108_atomic_spin_lock/main.sla --out /tmp/108_atomic_spin_lock.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/108_atomic_spin_lock/main.sla
```
