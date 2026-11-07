# 201 Pkg Manifest Basic

This slot now uses a real manifest-file reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a small surrogate observable instead of a true package-manifest parser.

- `main.rs`: Rust reference that reads `sa.pkg` and checks the required manifest fields.
- `main.sla`: current surrogate that only preserves a three-field count observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/201_pkg_manifest_basic/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/201_pkg_manifest_basic/main.sla --out /tmp/201_pkg_manifest_basic.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/201_pkg_manifest_basic/main.sla
```
