# 187 Opengl Context Swap

OpenGL-style context slot kept as an explicit surrogate.

- `main.rs`: Rust reference for the unsafe `gl_make_current` and `gl_swap_buffers` calls.
- `main.sla`: Sla surrogate using `ptr::null::<u8>()` and local extern stubs.

Because the Sla side does not bind a real external OpenGL context API, this slot should stay `❌` in `demos/rosetta/demo.md`.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/187_opengl_context_swap/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/187_opengl_context_swap/main.sla --out /tmp/187_opengl_context_swap.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/187_opengl_context_swap/main.sla
```
