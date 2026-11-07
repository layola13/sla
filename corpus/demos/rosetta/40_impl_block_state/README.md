# 040 Impl Block State

This directory matches the impl-state-update topic for the catalog slot.

- `main.rs`: Rust reference for the account-state update semantics used by this slot.
- `main.sla`: Sla companion for the same observable total, but it currently consumes `self` and returns a new `Account` instead of mutating through Rust's `&mut self` receiver, so this slot remains a non-1:1 surrogate.
