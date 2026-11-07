# 184 Pthread Spawn Join

Thread spawn-and-join demo that runs `worker(1)` on a spawned thread and returns the joined value `5`.

- `main.rs`: Rust reference using `thread::spawn(...).join().unwrap()`.
- `main.sla`: Sla companion for the worker spawn-and-join flow.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/184_pthread_spawn_join/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/184_pthread_spawn_join/main.sla --out /tmp/184_pthread_spawn_join.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/184_pthread_spawn_join/main.sla
```
