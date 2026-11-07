# 135 Async Streams

This directory now records the current async-stream gap honestly.

- `main.rs`: Rust reference using `impl Stream`, `next().await`, and accumulation across yielded items.
- `main.sla`: Sla surrogate that preserves only the three-item accumulation observable through explicit helper calls.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/135_async_streams/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/135_async_streams/main.sla --out /tmp/135_async_streams.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/135_async_streams/main.sla
```
