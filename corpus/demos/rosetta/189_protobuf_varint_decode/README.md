# 189 Protobuf Varint Decode

This slot keeps a small decoded varint observable as a plain integer.

- `main.rs`: Rust reference for decoding the varint into a plain integer.
- `main.sla`: Sla companion for the plain integer decode.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/189_protobuf_varint_decode/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/189_protobuf_varint_decode/main.sla --out /tmp/189_protobuf_varint_decode.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/189_protobuf_varint_decode/main.sla
```
