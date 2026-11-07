# 104 If Let Chains

This directory matches the if-let-chain catalog slot, combining multiple `Option` matches into one branch result.

- `main.rs`: Rust reference for chained `if let` matching.
- `main.sla`: Sla companion for chained `if let` matching.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/104_if_let_chains/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/104_if_let_chains/main.sla --out /tmp/104_if_let_chains.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/104_if_let_chains/main.sla
```
