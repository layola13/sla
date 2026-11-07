# 157 Aligned Alloc Simd

This directory keeps the aligned-allocation SIMD slot as an explicit surrogate.

- `main.rs`: Rust surrogate for the lane count implied by a 16-byte aligned `i32` SIMD-shaped chunk.
- `main.sla`: Sla companion for the same alignment-to-lane observable.

It is not a literal aligned allocation or SIMD intrinsic demo, so this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/157_aligned_alloc_simd/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/157_aligned_alloc_simd/main.sla --out /tmp/157_aligned_alloc_simd.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/157_aligned_alloc_simd/main.sla
```
