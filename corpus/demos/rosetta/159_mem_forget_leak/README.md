# 159 Mem Forget Leak

This directory matches the `mem::forget` leak/ownership-transfer catalog slot.

- `main.rs`: Rust reference for consuming ownership without running normal cleanup.
- `main.sla`: Sla companion for consuming ownership without running normal cleanup.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/159_mem_forget_leak/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/159_mem_forget_leak/main.sla --out /tmp/159_mem_forget_leak.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/159_mem_forget_leak/main.sla
```
