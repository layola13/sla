# 223 Mod Visibility Private

This slot now uses a real public/internal visibility fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a private-helper count instead of true visibility enforcement.

- `main.rs`: Rust reference that checks the public wrapper, internal bridge, and private detail module boundary.
- `main.sla`: current surrogate that only preserves a one-private-helper count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/223_mod_visibility_private/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/223_mod_visibility_private/main.sla --out /tmp/223_mod_visibility_private.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/223_mod_visibility_private/main.sla
```
