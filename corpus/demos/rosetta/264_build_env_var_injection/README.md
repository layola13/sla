# 264 Build Env Var Injection

This slot now uses a real fixture-backed environment-injection reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one profile-variable count instead of checking env profile selection and generated output.

- `main.rs`: Rust reference that reads `build/env.toml`, `env/dev.env`, `env/release.env`, and `generated/env_profile.sa`.
- `main.sla`: current surrogate that only preserves the injected-profile count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/264_build_env_var_injection/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/264_build_env_var_injection/main.sla --out /tmp/264_build_env_var_injection.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/264_build_env_var_injection/main.sla
```
