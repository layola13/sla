# 125 Once Cell Lazy

This directory now records the current once-cell gap honestly.

- `main.rs`: Rust reference using a real static `OnceLock` and `get_or_init(...)` reuse.
- `main.sla`: Sla surrogate using a local `ONCE_NEW()` handle and repeated `ONCE_GET_OR_INIT(...)` calls within one function.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/125_once_cell_lazy/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/125_once_cell_lazy/main.sla --out /tmp/125_once_cell_lazy.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/125_once_cell_lazy/main.sla
```
