# 225 Mod Namespace Prefix

This slot now uses a real namespace-prefix fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a namespace-plus-symbol count instead of true namespace lookup.

- `main.rs`: Rust reference that reads the `ns/prefix` aggregate and both prefixed symbol modules.
- `main.sla`: current surrogate that only preserves a namespace-plus-symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/225_mod_namespace_prefix/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/225_mod_namespace_prefix/main.sla --out /tmp/225_mod_namespace_prefix.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/225_mod_namespace_prefix/main.sla
```
