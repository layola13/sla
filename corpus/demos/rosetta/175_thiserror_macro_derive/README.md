# 175 Thiserror Macro Derive

This directory matches the thiserror-style derive catalog slot.

- `main.rs`: Rust reference for derived error-format observation.
- `main.sla`: Sla companion for derived error-format observation.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/175_thiserror_macro_derive/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/175_thiserror_macro_derive/main.sla --out /tmp/175_thiserror_macro_derive.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/175_thiserror_macro_derive/main.sla
```
