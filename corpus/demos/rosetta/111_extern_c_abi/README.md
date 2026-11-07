# 111 Extern C Abi

This directory matches the extern-C ABI catalog slot.

- `main.rs`: Rust reference for exporting and calling an `extern "C"` function.
- `main.sla`: Sla companion for exporting and calling an `extern "C"` function.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/111_extern_c_abi/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/111_extern_c_abi/main.sla --out /tmp/111_extern_c_abi.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/111_extern_c_abi/main.sla
```
