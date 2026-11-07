# 103 Labeled Break

This directory matches the labeled-break catalog slot, using nested-loop exit state as the observable behavior.

- `main.rs`: Rust reference for labeled loop termination.
- `main.sla`: Sla companion for labeled loop termination.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/103_labeled_break/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/103_labeled_break/main.sla --out /tmp/103_labeled_break.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/103_labeled_break/main.sla
```
