# 186 Sqlite C Api Binding

SQLite-style C ABI slot kept as an explicit surrogate.

- `main.rs`: Rust reference for the `extern "C"` row-pointer call.
- `main.sla`: Sla surrogate with a local `sqlite_insert` shim returning `row.count`.

Because the Sla side does not bind a real external SQLite C API, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/186_sqlite_c_api_binding/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/186_sqlite_c_api_binding/main.sla --out /tmp/186_sqlite_c_api_binding.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/186_sqlite_c_api_binding/main.sla
```
