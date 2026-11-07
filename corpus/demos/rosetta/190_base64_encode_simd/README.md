# 190 Base64 Encode Simd

This slot keeps the Base64 block-encode topic as an explicit partial-surrogate target.

- `main.rs` computes the four sextets, indexes the Base64 alphabet, and converts the `[u8; 4]` output to UTF-8 text.
- `main.sla` attempts to mirror the current Base64 encode flow with explicit `sa_std` imports and `encoded.iter().collect<String>()`.

The checked-in Sla source currently hits a local `u8` arithmetic/type-check gap before codegen, so this slot should stay `❌` in `demos/rosetta/demo.md` until the full encode path is accepted again.

Expected output:

```text
TWFu
```

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/190_base64_encode_simd/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/190_base64_encode_simd/main.sla --out /tmp/190_base64_encode_simd.sa
sa test /tmp/190_base64_encode_simd.sa --trace-panic
```
