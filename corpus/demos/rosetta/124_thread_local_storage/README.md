# 124 Thread Local Storage

This directory matches the thread-local storage catalog slot.

- `main.rs`: Rust reference for a TLS `Cell<i32>` slot that is read, incremented, and read again through `with(...)`.
- `main.sla`: Sla companion for a TLS `Cell<i32>` slot that is read, incremented, and read again through `with(...)`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/124_thread_local_storage/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/124_thread_local_storage/main.sla --out /tmp/124_thread_local_storage.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/124_thread_local_storage/main.sla
```
