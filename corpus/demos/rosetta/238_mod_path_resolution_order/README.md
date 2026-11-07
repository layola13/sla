# 238 Mod Path Resolution Order

This slot now uses a real ordered-path fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a priority-count observable instead of true path resolution ordering.

- `main.rs`: Rust reference that reads the aggregate path module plus first/second branch modules and checks their explicit order.
- `main.sla`: current surrogate that only preserves a one-priority count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/238_mod_path_resolution_order/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/238_mod_path_resolution_order/main.sla --out /tmp/238_mod_path_resolution_order.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/238_mod_path_resolution_order/main.sla
```
