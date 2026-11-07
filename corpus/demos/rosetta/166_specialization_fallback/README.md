# 166 Specialization Fallback

This directory keeps the specialization-fallback slot as an explicit surrogate.

- `main.rs`: Rust reference for `min_specialization` with a specialized `i32` branch and a fallback branch.
- `main.sla`: Sla surrogate for the specialized-plus-fallback observable total.

Because the Sla side does not execute real specialization resolution, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/166_specialization_fallback/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/166_specialization_fallback/main.sla --out /tmp/166_specialization_fallback.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/166_specialization_fallback/main.sla
```
