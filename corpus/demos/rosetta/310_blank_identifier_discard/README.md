# 310 Blank Identifier Discard

This directory demonstrates the discard sink `_` for owned values and destructuring.

- `main.sla`: Sla demo that discards owned temporaries, tuple slots, and slice-rest intermediates without polluting scope.
- `main.sa`: lowered SA fixture that keeps the same observable result while showing the cleanup path.

## Verified

```bash
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/310_blank_identifier_discard/main.sla
```
