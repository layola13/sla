# 170 Marker Traits

This directory matches the marker-traits catalog slot.

- `main.rs`: Rust reference for a marker-driven processing observable.
- `main.sla`: Sla companion for a marker-driven processing observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/170_marker_traits/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/170_marker_traits/main.sla --out /tmp/170_marker_traits.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/170_marker_traits/main.sla
```
