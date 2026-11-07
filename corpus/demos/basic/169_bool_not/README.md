# 169 Bool Not

对标 `tsgosa/demos/1220_double_negation` 的布尔部。

- `!`/`!!` 只容布尔操作数（`!5` 按严格布尔拒识，见 129；
  整数取反语义用 `== 0` / `!= 0` 表达）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/169_bool_not/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/169_bool_not/main.sla
```
