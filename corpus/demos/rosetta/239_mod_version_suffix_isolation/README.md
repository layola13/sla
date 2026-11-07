# 239 Mod Version Suffix Isolation

This slot now uses a real version-suffixed fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a variant-count observable instead of true suffix-isolated module resolution.

- `main.rs`: Rust reference that reads the `versions/v1` and `versions/v2` module/layout/seed trees and checks they stay isolated.
- `main.sla`: current surrogate that only preserves a two-variant count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/239_mod_version_suffix_isolation/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/239_mod_version_suffix_isolation/main.sla --out /tmp/239_mod_version_suffix_isolation.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/239_mod_version_suffix_isolation/main.sla
```
