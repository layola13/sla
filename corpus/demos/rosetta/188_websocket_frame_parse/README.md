# 188 Websocket Frame Parse

This slot checks a small frame header predicate through explicit boolean tests.

- `main.rs`: Rust reference for the frame-header boolean checks.
- `main.sla`: Sla companion for the frame-header boolean checks.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/188_websocket_frame_parse/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/188_websocket_frame_parse/main.sla --out /tmp/188_websocket_frame_parse.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/188_websocket_frame_parse/main.sla
```
