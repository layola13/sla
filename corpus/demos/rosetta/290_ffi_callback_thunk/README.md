# 290 Ffi Callback Thunk

This slot now uses a real fixture-backed FFI callback thunk registration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/callback_thunk.*`, `ffi/callback.sai`, callback registry header, loader note, and thunk docs.
- `main.sla`: current surrogate that only preserves the callback-thunk count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/290_ffi_callback_thunk/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/290_ffi_callback_thunk/main.sla --out /tmp/290_ffi_callback_thunk.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/290_ffi_callback_thunk/main.sla
```
