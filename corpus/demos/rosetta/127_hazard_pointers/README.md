# 127 Hazard Pointers

This directory now records the current hazard-pointer surrogate honestly.

- `main.rs`: local Rust surrogate that loads a raw pointer, mirrors it into a second atomic slot, and checks pointer equality before dereference.
- `main.sla`: matching Sla surrogate for the same protected-pointer observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/127_hazard_pointers/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/127_hazard_pointers/main.sla --out /tmp/127_hazard_pointers.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/127_hazard_pointers/main.sla
```
