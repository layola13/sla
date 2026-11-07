# 080 Workflow

This directory matches the workflow-progress topic for the catalog slot.

- `main.rs`: Rust reference for the completed-step count used by this slot.
- `main.sla`: Sla companion for the completed-step count used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/80_workflow/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/80_workflow/main.sla --out /tmp/80_workflow.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/80_workflow/main.sla
```
