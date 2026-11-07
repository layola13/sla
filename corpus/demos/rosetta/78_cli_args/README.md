# 078 Cli Args

This directory matches the CLI-decision topic for the catalog slot.

- `main.rs`: Rust reference for the command/release semantics used by this slot.
- `main.sla`: Sla companion for the command/release semantics used by this slot.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/78_cli_args/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/78_cli_args/main.sla --out /tmp/78_cli_args.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/78_cli_args/main.sla
```
