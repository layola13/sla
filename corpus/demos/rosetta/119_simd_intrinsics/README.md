# 119 Simd Intrinsics

This directory now records the current SIMD surrogate honestly.

- `main.rs`: local Rust surrogate that sums four lanes across cfg branches without using real SIMD intrinsics.
- `main.sla`: matching Sla surrogate for the same lane-sum observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/119_simd_intrinsics/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/119_simd_intrinsics/main.sla --out /tmp/119_simd_intrinsics.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/119_simd_intrinsics/main.sla
```
