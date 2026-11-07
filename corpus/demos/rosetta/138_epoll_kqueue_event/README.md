# 138 Epoll Kqueue Event

This directory matches the epoll/kqueue event catalog slot.

- `main.rs`: Rust reference for detecting a ready event.
- `main.sla`: Sla companion for detecting a ready event.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/138_epoll_kqueue_event/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/138_epoll_kqueue_event/main.sla --out /tmp/138_epoll_kqueue_event.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/138_epoll_kqueue_event/main.sla
```
