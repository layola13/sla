# 141 Vec Pop

对标 `tsgosa/demos/507_arr_api` 的 pop 部（`2,1,1`）。

- `pop()` 返回 `Option`（空表 `None`），此处以 `unwrap()` 取值
  （与 15 的 Option 口径一致；`map/filter/join` 等方法暂无，见 81/116）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/141_vec_pop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/141_vec_pop/main.sla
```
