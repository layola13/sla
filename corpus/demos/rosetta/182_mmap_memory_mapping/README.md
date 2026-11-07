# 182 Mmap Memory Mapping

File-backed mapping setup demo that opens `/dev/zero`, reads its descriptor, and derives a mapped value from it.

- `main.rs`: Rust reference for the descriptor flow.
- `main.sla`: Sla companion using `File::open(...).unwrap()` and `as_raw_fd()`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/182_mmap_memory_mapping/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/182_mmap_memory_mapping/main.sla --out /tmp/182_mmap_memory_mapping.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/182_mmap_memory_mapping/main.sla
```
