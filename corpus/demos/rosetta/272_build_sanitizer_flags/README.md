# 272 Build Sanitizer Flags

This slot now uses a real fixture-backed sanitizer-config reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves flag counts instead of checking requested sanitizers, resolved flags, and generated output.

- `main.rs`: Rust reference that reads `build/sanitizer.toml`, `config/sanitizer/flags.toml`, and `generated/sanitizer/flags.sa`.
- `main.sla`: current surrogate that only preserves the sanitizer-flag count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/272_build_sanitizer_flags/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/272_build_sanitizer_flags/main.sla --out /tmp/272_build_sanitizer_flags.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/272_build_sanitizer_flags/main.sla
```
