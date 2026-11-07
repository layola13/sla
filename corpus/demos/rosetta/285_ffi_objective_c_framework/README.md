# 285 Ffi Objective C Framework

This slot now uses a real fixture-backed Objective-C framework FFI linkage reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `bridge/objc_gate.*`, `ffi/objective_c.sai`, framework header, modulemap, and binary note.
- `main.sla`: current surrogate that only preserves the framework count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/285_ffi_objective_c_framework/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/285_ffi_objective_c_framework/main.sla --out /tmp/285_ffi_objective_c_framework.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/285_ffi_objective_c_framework/main.sla
```
