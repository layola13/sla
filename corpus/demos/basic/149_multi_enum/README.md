# 149 Multi Enum

对标 `tsgosa/demos/842_top_multi_enum`（`1+2=3`）。

- 枚举支持显式判别值（`A = 1`，探针已验证）；`match` 须以变量
  作 scrutinee（直接 `match E::A` 不可解析），此处以函数参数过渡。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/149_multi_enum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/149_multi_enum/main.sla
```
