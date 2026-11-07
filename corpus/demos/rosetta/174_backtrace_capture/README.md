# 174 Backtrace Capture

This directory matches the backtrace-capture catalog slot.

- `main.rs`: Rust reference for a backtrace-depth observable.
- `main.sla`: Sla companion for a backtrace-depth observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/174_backtrace_capture/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/174_backtrace_capture/main.sla --out /tmp/174_backtrace_capture.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/174_backtrace_capture/main.sla
```
