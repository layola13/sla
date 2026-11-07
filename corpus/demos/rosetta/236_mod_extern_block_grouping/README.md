# 236 Mod Extern Block Grouping

This slot now uses a real grouped-extern fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a grouped-symbol count instead of true extern block grouping semantics.

- `main.rs`: Rust reference that reads the grouped iface, layout, bridge, and seed modules.
- `main.sla`: current surrogate that only preserves a two-symbol count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/236_mod_extern_block_grouping/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/236_mod_extern_block_grouping/main.sla --out /tmp/236_mod_extern_block_grouping.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/236_mod_extern_block_grouping/main.sla
```
