# 263 Build Asset Bundling

This slot now uses a real fixture-backed asset-bundling reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves artifact counts instead of checking the asset manifest, input files, and generated bundle.

- `main.rs`: Rust reference that reads `bundle/manifest.toml`, `assets/text/*.txt`, and `generated/asset_bundle.sa`.
- `main.sla`: current surrogate that only preserves the packaged-artifact count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/263_build_asset_bundling/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/263_build_asset_bundling/main.sla --out /tmp/263_build_asset_bundling.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/263_build_asset_bundling/main.sla
```
