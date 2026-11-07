# 112 Vec Grow

对标 `tsgosa/demos/269_arr_rebind`（长 2→4，首尾和 8）。

- SLA 无 `concat` 返回新数组，生长语义用 `Vec::push` 表达
  （与 39/53/103 的 Vec 口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/112_vec_grow/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/112_vec_grow/main.sla
```
