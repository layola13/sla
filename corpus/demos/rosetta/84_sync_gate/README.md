# 084 Sync Gate

This directory now records the current sync-gate mismatch honestly.

- `main.rs`: Rust reference for gate-state code semantics where `arrived < required` takes precedence over `drained`.
- `main.sla`: current Sla companion shape, whose branch order checks `drained` first and therefore does not preserve the same semantics for all states.
