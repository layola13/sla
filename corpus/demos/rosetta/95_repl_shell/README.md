# 095 Repl Shell

This directory matches the REPL command-dispatch topic for the catalog slot, including command mode and expression fallback handling.

- `main.rs`: Rust reference for the line-evaluation semantics used by this slot.
- `main.sla`: Sla companion for the line-evaluation semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/95_repl_shell/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/95_repl_shell/main.sla --out /tmp/95_repl_shell.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/95_repl_shell/main.sla
```
