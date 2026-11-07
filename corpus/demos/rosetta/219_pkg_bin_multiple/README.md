# 219 Pkg Bin Multiple

This slot now uses a real multi-bin fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a binary-count observable instead of true multiple binary target handling.

- `main.rs`: Rust reference that reads the root manifest plus both sibling bin modules and their helper constants.
- `main.sla`: current surrogate that only preserves a two-bin count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/219_pkg_bin_multiple/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/219_pkg_bin_multiple/main.sla --out /tmp/219_pkg_bin_multiple.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/219_pkg_bin_multiple/main.sla
```
