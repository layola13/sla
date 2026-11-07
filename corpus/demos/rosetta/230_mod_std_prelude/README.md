# 230 Mod Std Prelude

This slot now uses a real local-prelude fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a prelude-symbol count instead of true std prelude import behavior.

- `main.rs`: Rust reference that reads the prelude aggregate, interface, layout, and core seed module.
- `main.sla`: current surrogate that only preserves a three-symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/230_mod_std_prelude/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/230_mod_std_prelude/main.sla --out /tmp/230_mod_std_prelude.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/230_mod_std_prelude/main.sla
```
