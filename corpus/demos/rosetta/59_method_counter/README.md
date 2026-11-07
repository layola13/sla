# 059 Method Counter

This directory now records the current method-counter surrogate honestly.

- `main.rs`: Rust reference using a tuple struct `Counter(i32)` and a real `&mut self` method.
- `main.sla`: Sla surrogate that preserves the same increment observable through a `Cell<i32>` field. `inc` takes `&self` (Sla has no `&mut self` yet; `&self` covers read and write), so `counter` stays usable after the call.
