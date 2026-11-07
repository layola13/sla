# 200 Sa Asm Quine

This slot keeps the SA-ASM quine theme observable as a fixed source-snippet surrogate.

- `main.rs`: Rust reference for printing the source text via `include_str!`.
- `main.sla`: Sla surrogate that prints a fixed source snippet and checks its length.

Because the Sla side does not emit its own full source text, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/200_sa_asm_quine/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/200_sa_asm_quine/main.sla --out /tmp/200_sa_asm_quine.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/200_sa_asm_quine/main.sla
```
