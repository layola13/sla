# 081 Kv Store

This directory matches the key-value store topic for the catalog slot.

- `main.rs`: Rust reference for the BTreeMap insert/index semantics used by this slot.
- `main.sla`: Sla companion for the BTreeMap insert/index semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/81_kv_store/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/81_kv_store/main.sla --out /tmp/81_kv_store.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/81_kv_store/main.sla
```
