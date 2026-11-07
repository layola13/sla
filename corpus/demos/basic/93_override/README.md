# 93 Override

对标 `tsgosa/demos/104_override`（`1, 2`）。

- SLA 的 `impl` 暂无继承语义，用两个独立结构体各自实现 `voice` 表达同一输出。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/93_override/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/93_override/main.sla
```
