# 172 Eyre Color Eyre

This directory now records the current `eyre` / `color-eyre` gap honestly.

- `main.rs`: local Rust surrogate that materializes a context string and measures its length.
- `main.sla`: matching Sla surrogate for the same context-string observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/172_eyre_color_eyre/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/172_eyre_color_eyre/main.sla --out /tmp/172_eyre_color_eyre.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/172_eyre_color_eyre/main.sla
```
