# 146 Never Type Fallback

This directory matches the never-type fallback catalog slot.

- `main.rs`: Rust reference for fallback behavior around a never-type path.
- `main.sla`: Sla companion for fallback behavior around a never-type path.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/146_never_type_fallback/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/146_never_type_fallback/main.sla --out /tmp/146_never_type_fallback.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/146_never_type_fallback/main.sla
```
