# 168 Type Alias Impl Trait

This directory keeps the type-alias-`impl Trait` slot as an explicit surrogate.

- `main.rs`: Rust reference for a real `type MyIter = impl Iterator<Item = i32>` producer.
- `main.sla`: Sla surrogate that preserves the helper-and-bind shape with a concrete array producer.

Because the Sla side does not model `impl Trait` in a type alias, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/168_type_alias_impl_trait/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/168_type_alias_impl_trait/main.sla --out /tmp/168_type_alias_impl_trait.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/168_type_alias_impl_trait/main.sla
```
