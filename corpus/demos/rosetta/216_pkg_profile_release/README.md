# 216 Pkg Profile Release

This slot now uses a real release-profile fixture on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a release-profile count instead of true profile resolution.

- `main.rs`: Rust reference that reads the profile tree, helper constants, and package metadata.
- `main.sla`: current surrogate that only preserves a one-release-profile count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/216_pkg_profile_release/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/216_pkg_profile_release/main.sla --out /tmp/216_pkg_profile_release.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/216_pkg_profile_release/main.sla
```
