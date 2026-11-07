# 085 Scheduler Tree

This slot models a small scheduler tree that reduces a root task with the heaviest child path.

- `main.rs`: Rust reference for the tree-shaped critical-path reduction.
- `main.sla`: Sla companion for the tree-shaped critical-path reduction.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/85_scheduler_tree/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/85_scheduler_tree/main.sla --out /tmp/85_scheduler_tree.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/85_scheduler_tree/main.sla
```
