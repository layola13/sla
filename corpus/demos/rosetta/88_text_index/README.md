# 088 Text Index

This slot models a text index lookup that counts token hits across a small corpus.

- `main.rs`: Rust reference for the index lookup.
- `main.sla`: Sla companion for the index lookup.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/88_text_index/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/88_text_index/main.sla --out /tmp/88_text_index.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/88_text_index/main.sla
```
