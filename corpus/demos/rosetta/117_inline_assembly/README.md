# 117 Inline Assembly

This directory now records the current inline-assembly surrogate honestly.

- `main.rs`: local Rust surrogate using a no-op inline assembly escape that preserves the observed value.
- `main.sla`: matching Sla surrogate for the same value-stable assembly escape observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/117_inline_assembly/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/117_inline_assembly/main.sla --out /tmp/117_inline_assembly.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/117_inline_assembly/main.sla
```
