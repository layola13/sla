# 297 Eco Game Engine Ecs

This slot now uses a real fixture-backed game-engine ECS integration reference on the Rust side, but it should still be treated as `❌` because the Sla side only preserves a count-style observable instead of checking the full FFI/ecosystem fixture graph.

- `main.rs`: Rust reference that reads `engine/world.*`, ECS docs, scene assets, and scene metadata.
- `main.sla`: current surrogate that only preserves the component-type count.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/297_eco_game_engine_ecs/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/297_eco_game_engine_ecs/main.sla --out /tmp/297_eco_game_engine_ecs.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/297_eco_game_engine_ecs/main.sla
```
