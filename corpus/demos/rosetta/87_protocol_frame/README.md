# 087 Protocol Frame

This slot models a protocol frame with a fixed header and payload checksum.

- `main.rs`: Rust reference for the frame validation.
- `main.sla`: Sla companion for the frame validation.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/87_protocol_frame/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/87_protocol_frame/main.sla --out /tmp/87_protocol_frame.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/87_protocol_frame/main.sla
```
