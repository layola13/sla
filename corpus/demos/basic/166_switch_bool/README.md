# 166 Switch Bool

对标 `tsgosa/demos/1099_switch_bool`（`0,1`）。

- `true`/`false` 作字面量臂双后端正常（探针 + exe 核对；
  条件表达式臂见缺口 #22，此处只用字面量臂）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/166_switch_bool/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/166_switch_bool/main.sla
```
