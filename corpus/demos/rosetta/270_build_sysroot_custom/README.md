# 270 Build Sysroot Custom

This slot now uses a real fixture-backed custom-sysroot reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves layer counts instead of checking sysroot config, headers, and generated layout.

- `main.rs`: Rust reference that reads `build/sysroot.toml`, `include/sysroot/*.h`, and `generated/sysroot/layout.sa`.
- `main.sla`: current surrogate that only preserves the sysroot-layer count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/270_build_sysroot_custom/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/270_build_sysroot_custom/main.sla --out /tmp/270_build_sysroot_custom.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/270_build_sysroot_custom/main.sla
```
