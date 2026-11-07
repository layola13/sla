# 121 Rwlock Reader Writer

This slot models `Arc<RwLock<i32>>`, a spawned reader, joined result, exclusive write, and final read.

- `main.rs`: Rust reference using `Arc<RwLock<i32>>`, a spawned reader, joined result, exclusive write, and final read.
- `main.sla`: Sla companion for the same flow, with minimal read/write staging that is currently required for the checked dereference and update path to type-check locally.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/121_rwlock_reader_writer/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/121_rwlock_reader_writer/main.sla --out /tmp/121_rwlock_reader_writer.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/121_rwlock_reader_writer/main.sla
```
