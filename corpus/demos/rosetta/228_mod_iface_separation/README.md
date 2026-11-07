# 228 Mod Iface Separation

This slot now uses a real interface/layout/implementation fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a layer count instead of true interface/implementation contract handling.

- `main.rs`: Rust reference that reads `api/contract.sai`, `layout/contract.sal`, and `impl/contract.sa` separately.
- `main.sla`: current surrogate that only preserves a two-layer count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/228_mod_iface_separation/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/228_mod_iface_separation/main.sla --out /tmp/228_mod_iface_separation.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/228_mod_iface_separation/main.sla
```
