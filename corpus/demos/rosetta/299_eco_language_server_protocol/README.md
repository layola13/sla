# 299 Eco Language Server Protocol

This slot now uses a real fixture-backed language-server protocol integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `lsp/server.*`, server docs, protocol capabilities, and LSP JSON.
- `main.sla`: current surrogate that only preserves the message-kind count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/299_eco_language_server_protocol/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/299_eco_language_server_protocol/main.sla --out /tmp/299_eco_language_server_protocol.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/299_eco_language_server_protocol/main.sla
```
