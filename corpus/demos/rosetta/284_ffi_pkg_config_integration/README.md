# 284 Ffi Pkg Config Integration

This slot now uses a real fixture-backed pkg-config FFI integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `config/pkg-config.toml`, `bridge/pkg_config_gate.*`, `ffi/pkg_config.sai`, host header, and `.pc` metadata.
- `main.sla`: current surrogate that only preserves the include/library path count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/284_ffi_pkg_config_integration/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/284_ffi_pkg_config_integration/main.sla --out /tmp/284_ffi_pkg_config_integration.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/284_ffi_pkg_config_integration/main.sla
```
