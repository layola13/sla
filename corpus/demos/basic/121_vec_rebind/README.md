# 121 Vec Rebind

对标 `tsgosa/demos/381_arr_clear`（`3,0,1,9`）。

- SLA 无 `Vec.clear()`（`Undefined call`），亦无 `length=` setter，
  清空语义用重建绑定 `a = Vec::new()` 表达（与 269 原用例的重绑定同构）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/121_vec_rebind/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/121_vec_rebind/main.sla
```
