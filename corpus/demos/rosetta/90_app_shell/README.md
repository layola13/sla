# 090 App Shell

This slot models a small app shell dispatcher that selects an exit code from command and config state.

- `main.rs`: Rust reference for the command dispatch.
- `main.sla`: Sla companion for the command dispatch.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/90_app_shell/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/90_app_shell/main.sla --out /tmp/90_app_shell.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/90_app_shell/main.sla
```
