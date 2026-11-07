# 185 Dynamic Lib Dlopen

This demo keeps the `extern "C"` pointer-return and unsafe-call shape of `dlopen` / `dlclose` inside the current Sla compiler surface.

- `main.rs`: the Rust catalog reference using libc-style dynamic library entry points.
- `main.sla`: Deterministic Sla companion for local extern pointer-call lowering through `@no_mangle extern "C"` shims.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/185_dynamic_lib_dlopen/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/185_dynamic_lib_dlopen/main.sla --out /tmp/185_dynamic_lib_dlopen.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/185_dynamic_lib_dlopen/main.sla
```
