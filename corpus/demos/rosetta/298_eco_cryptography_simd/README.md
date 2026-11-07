# 298 Eco Cryptography Simd

This slot now uses a real fixture-backed cryptography SIMD integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `crypto/hash.*`, SIMD docs, crypto header, and benchmark metadata.
- `main.sla`: current surrogate that only preserves the SIMD-lane count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/298_eco_cryptography_simd/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/298_eco_cryptography_simd/main.sla --out /tmp/298_eco_cryptography_simd.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/298_eco_cryptography_simd/main.sla
```
