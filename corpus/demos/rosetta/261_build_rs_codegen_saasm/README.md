# 261 Build Rs Codegen Saasm

This slot now uses a real fixture-backed codegen reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves one emitted-module count instead of checking the build manifest, plan, and generated SA-ASM output.

- `main.rs`: Rust reference that reads `build/codegen.toml`, `build/codegen-plan.txt`, and `generated/codegen.sa`.
- `main.sla`: current surrogate that only preserves the emitted-module count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/261_build_rs_codegen_saasm/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/261_build_rs_codegen_saasm/main.sla --out /tmp/261_build_rs_codegen_saasm.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/261_build_rs_codegen_saasm/main.sla
```
