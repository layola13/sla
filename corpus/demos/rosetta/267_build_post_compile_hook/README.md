# 267 Build Post Compile Hook

This slot now uses a real fixture-backed post-compile hook reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one artifact count instead of checking post-hook config, report manifest, script, and generated output.

- `main.rs`: Rust reference that reads `build/post-hooks.toml`, `hooks/post-compile.sh`, `artifacts/post-build/*`, and `generated/postcompile.sa`.
- `main.sla`: current surrogate that only preserves the post-compile artifact count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/267_build_post_compile_hook/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/267_build_post_compile_hook/main.sla --out /tmp/267_build_post_compile_hook.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/267_build_post_compile_hook/main.sla
```
