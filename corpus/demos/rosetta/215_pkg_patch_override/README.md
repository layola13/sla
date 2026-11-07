# 215 Pkg Patch Override

This slot now uses a real patch-override fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a patched-source flag instead of true package patch resolution.

- `main.rs`: Rust reference that reads the package manifest, upstream helper, override helper, and patch-bias definition.
- `main.sla`: current surrogate that only preserves a one-patch-applied flag.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/215_pkg_patch_override/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/215_pkg_patch_override/main.sla --out /tmp/215_pkg_patch_override.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/215_pkg_patch_override/main.sla
```
