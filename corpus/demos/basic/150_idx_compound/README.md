# 150 Idx Compound

对标 `tsgosa/demos/908_arr_compound` + `940_compound_var_idx`（`6,13`）。

- 下标作复合赋值目标双后端正常（字面量下标 + 变量下标双覆盖；
  字段目标见 #12，`<<=` 等无 token 见 119）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/150_idx_compound/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/150_idx_compound/main.sla
```
