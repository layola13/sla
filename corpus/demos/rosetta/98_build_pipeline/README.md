# 098 Build Pipeline

This directory matches the build-artifact gating topic for the catalog slot, combining compile, test, package, and docs stages.

- `main.rs`: Rust reference for the compiled/tests/docs artifact-count semantics used by this slot.
- `main.sla`: Sla companion for the compiled/tests/docs artifact-count semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/98_build_pipeline/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/98_build_pipeline/main.sla --out /tmp/98_build_pipeline.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/98_build_pipeline/main.sla
```
