# 089 Job Queue

This slot currently records the FIFO queue example honestly rather than pretending the checked-in helper shape is literal 1:1.

- `main.rs`: Rust reference for the queue pop semantics.
- `main.sla`: current Sla companion, but the main path routes the queue flow through `first_job_score()` instead of keeping the queue operations directly in `main` like Rust.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/89_job_queue/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/89_job_queue/main.sla --out /tmp/89_job_queue.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/89_job_queue/main.sla
```
