# 181 File Descriptor Raii

File-open RAII demo that opens `/dev/null`, reads its raw file descriptor, and returns it.

- `main.rs`: Rust reference for the file-handle lifecycle.
- `main.sla`: Sla companion using `File::open(...).unwrap()` and `as_raw_fd()`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/181_file_descriptor_raii/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/181_file_descriptor_raii/main.sla --out /tmp/181_file_descriptor_raii.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/181_file_descriptor_raii/main.sla
```
