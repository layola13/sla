# 196 Lto Link Time Opt

This slot keeps the link-time-optimization theme observable as a combined hot-path and cold-path call total.

- `main.rs`: Rust reference for the hot-path and cold-path call total.
- `main.sla`: Sla companion for the combined hot-path and cold-path call total.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/196_lto_link_time_opt/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/196_lto_link_time_opt/main.sla --out /tmp/196_lto_link_time_opt.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/196_lto_link_time_opt/main.sla
```
