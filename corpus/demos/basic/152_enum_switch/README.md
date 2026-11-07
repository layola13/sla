# 152 Enum Switch

对标 `tsgosa/demos/943_enum_switch`（`2,1`）。

- 与 135/149 的 `match` 消费互补：`switch` 须带 `default` 臂
  （写法见 13/33）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/152_enum_switch/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/152_enum_switch/main.sla
```
