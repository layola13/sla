# 132 Pinning And Unpin

This directory now records the current pinning surrogate honestly.

- `main.rs`: Rust reference for observing stable address identity through `Pin`, `Pin::as_ref`, and `get_ref()`.
- `main.sla`: Sla surrogate that preserves the stable-address observable through the current local pin helper surface.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/132_pinning_and_unpin/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/132_pinning_and_unpin/main.sla --out /tmp/132_pinning_and_unpin.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/132_pinning_and_unpin/main.sla
```
