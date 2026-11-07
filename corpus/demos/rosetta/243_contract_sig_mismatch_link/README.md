# 243 Contract Sig Mismatch Link

This slot now uses a real fixture-backed signature-mismatch reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves an argument-count delta instead of exercising the broken link edge.

- `main.rs`: Rust reference that reads the `i32` target, slot layout, and pointer-passing broken consumer.
- `main.sla`: current surrogate that only preserves the one-mismatch count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/243_contract_sig_mismatch_link/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/243_contract_sig_mismatch_link/main.sla --out /tmp/243_contract_sig_mismatch_link.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/243_contract_sig_mismatch_link/main.sla
```
