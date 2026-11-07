# 115 Opaque Pointers

This directory now records the current opaque-pointer surrogate honestly.

- `main.rs`: Rust reference for passing a null opaque pointer through an FFI-shaped call.
- `main.sla`: matching Sla surrogate for the same null opaque-pointer call observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/115_opaque_pointers/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/115_opaque_pointers/main.sla --out /tmp/115_opaque_pointers.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/115_opaque_pointers/main.sla
```
