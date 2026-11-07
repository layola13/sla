# 227 Mod Shadowing Prevention

This slot now uses a real duplicate-layout fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a diagnostic count instead of true shadowing rejection.

- `main.rs`: Rust reference that reads the registry plus left/right branches with conflicting `SHADOW_SIZE` definitions.
- `main.sla`: current surrogate that only preserves a one-shadowing-diagnostic count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/227_mod_shadowing_prevention/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/227_mod_shadowing_prevention/main.sla --out /tmp/227_mod_shadowing_prevention.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/227_mod_shadowing_prevention/main.sla
```
