# 101 Custom Drop

This directory matches the custom-drop catalog slot, using guard-style cleanup accounting as the observable behavior.

- `main.rs`: Rust reference for drop-order tallying around scoped guards.
- `main.sla`: Sla companion for drop-order tallying around scoped guards.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/101_custom_drop/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/101_custom_drop/main.sla --out /tmp/101_custom_drop.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/101_custom_drop/main.sla
```
