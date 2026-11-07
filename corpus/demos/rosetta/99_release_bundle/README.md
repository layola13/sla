# 099 Release Bundle

This directory matches the release-readiness topic for the catalog slot, combining binary, config, checksum, and signature manifest requirements.

- `main.rs`: Rust reference for the bundle-manifest readiness semantics used by this slot.
- `main.sla`: Sla companion for the bundle-manifest readiness semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/99_release_bundle/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/99_release_bundle/main.sla --out /tmp/99_release_bundle.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/99_release_bundle/main.sla
```
