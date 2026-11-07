# 133 Select Macro Race

This directory now records the current `select!` gap honestly.

- `main.rs`: Rust reference using a real biased `tokio::select!` over three async branches.
- `main.sla`: Sla surrogate that preserves only the chosen-value observable through a sequential await plus helper path.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/133_select_macro_race/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/133_select_macro_race/main.sla --out /tmp/133_select_macro_race.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/133_select_macro_race/main.sla
```
