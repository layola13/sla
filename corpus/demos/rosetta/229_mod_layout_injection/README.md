# 229 Mod Layout Injection

This slot now uses a real layout-injection fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves an injected-field count instead of true layout-driven wrapper behavior.

- `main.rs`: Rust reference that reads the FFI contract, record layout, and wrapper code that uses the injected offsets.
- `main.sla`: current surrogate that only preserves a one-field count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/229_mod_layout_injection/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/229_mod_layout_injection/main.sla --out /tmp/229_mod_layout_injection.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/229_mod_layout_injection/main.sla
```
