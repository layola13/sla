# 164 Trait Upcasting

This directory keeps the trait-upcasting slot as an explicit partial surrogate.

- `main.rs`: Rust reference for the current local `A`/`B` trait sum observable.
- `main.sla`: Sla attempt at the same supertrait/upcast observable.

A focused local smoke still shows that the simpler dyn-dispatch subset works, but the fuller `dyn B` supertrait/upcast arithmetic path in this checked-in demo currently fails in type checking, so this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/164_trait_upcasting/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/164_trait_upcasting/main.sla --out /tmp/164_trait_upcasting.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/164_trait_upcasting/main.sla
```
