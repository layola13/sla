# 137 Io Uring Submission

This directory matches the io_uring submission catalog slot.

- `main.rs`: Rust reference for building a submission depth observable.
- `main.sla`: Sla companion for building a submission depth observable.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/137_io_uring_submission/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/137_io_uring_submission/main.sla --out /tmp/137_io_uring_submission.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/137_io_uring_submission/main.sla
```
