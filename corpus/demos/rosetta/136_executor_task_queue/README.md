# 136 Executor Task Queue

This directory matches the executor task queue catalog slot.

- `main.rs`: Rust reference for queuing and running executable tasks.
- `main.sla`: Sla companion for queuing and running executable tasks.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/136_executor_task_queue/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/136_executor_task_queue/main.sla --out /tmp/136_executor_task_queue.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/136_executor_task_queue/main.sla
```
