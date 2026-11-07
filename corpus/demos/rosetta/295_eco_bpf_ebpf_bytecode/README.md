# 295 Eco Bpf Ebpf Bytecode

This slot now uses a real fixture-backed eBPF bytecode integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `guest/program.*`, bytecode docs, attach metadata, and pin path.
- `main.sla`: current surrogate that only preserves the instruction-kind count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/295_eco_bpf_ebpf_bytecode/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/295_eco_bpf_ebpf_bytecode/main.sla --out /tmp/295_eco_bpf_ebpf_bytecode.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/295_eco_bpf_ebpf_bytecode/main.sla
```
