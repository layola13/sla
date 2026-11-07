# 240 Mod Entry Point Override

This slot now uses a real default/override entry fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves an override-selected count instead of true entry-point override behavior.

- `main.rs`: Rust reference that reads the default and override entry trees and checks the aggregate selects the override branch.
- `main.sla`: current surrogate that only preserves a one-override count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/240_mod_entry_point_override/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/240_mod_entry_point_override/main.sla --out /tmp/240_mod_entry_point_override.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/240_mod_entry_point_override/main.sla
```
