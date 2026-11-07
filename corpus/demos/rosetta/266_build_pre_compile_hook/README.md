# 266 Build Pre Compile Hook

This slot now uses a real fixture-backed pre-compile hook reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one phase count instead of checking hook config, script, notes, and generated output.

- `main.rs`: Rust reference that reads `build/pre-hooks.toml`, `hooks/pre-compile.sh`, `hooks/pre-compile.txt`, and `generated/precompile.sa`.
- `main.sla`: current surrogate that only preserves the pre-compile phase count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/266_build_pre_compile_hook/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/266_build_pre_compile_hook/main.sla --out /tmp/266_build_pre_compile_hook.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/266_build_pre_compile_hook/main.sla
```
