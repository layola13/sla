# 096 Task Orchestrator

This slot models a task score gated by dependencies and cooldown state.

- `main.rs`: Rust reference for the dependency-and-retry adjusted task score semantics used by this slot.
- `main.sla`: Sla companion for the same task scoring semantics, with a minimal field-load staging that is currently required for the checked arithmetic path to type-check locally.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/96_task_orchestrator/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/96_task_orchestrator/main.sla --out /tmp/96_task_orchestrator.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/96_task_orchestrator/main.sla
```
