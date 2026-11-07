# 144 Phantom Data Marker

This directory keeps the phantom-data marker slot as an explicit surrogate.

- `main.rs`: Rust reference for `Wrapper<T>` carrying `PhantomData<T>` and a typed `Wrapper<i64>` value.
- `main.sla`: Sla surrogate that preserves the `id` observable without claiming support for the full phantom generic shape.

Because the current Sla path does not accept the typed phantom-parameter struct-literal shape here, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/144_phantom_data_marker/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/144_phantom_data_marker/main.sla --out /tmp/144_phantom_data_marker.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/144_phantom_data_marker/main.sla
```
