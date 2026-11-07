# 199 Address Sanitizer Asan

This slot keeps the address-sanitizer theme observable as the sum of the first and last buffer-edge bytes.

- `main.rs`: Rust reference for the buffer-edge byte sum.
- `main.sla`: Sla companion for the buffer-edge byte sum.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/199_address_sanitizer_asan/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/199_address_sanitizer_asan/main.sla --out /tmp/199_address_sanitizer_asan.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/199_address_sanitizer_asan/main.sla
```
