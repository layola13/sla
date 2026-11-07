# 198 Clz32

对标 `tsgosa/demos/1438_clz32`（`clz32(1)=31`，另断言 `clz32(16)=27`）。

- SLA 无 `Math.clz32`，用 `>>` 折半循环手写（与 178 同机制）。
- `&` 须加括号（见 145）；`1<<31` 为 64 位正数 `2147483648`（见 171）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/198_clz32/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/198_clz32/main.sla
```
