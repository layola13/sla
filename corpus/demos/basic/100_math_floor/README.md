# 100 Math Floor

对标 `tsgosa/demos/31_math`（`5`，`7`）。

- `abs` 用分支表达；`floor` 用 `as i32` 截断表达（探针已验证 `7.9→7`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/100_math_floor/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/100_math_floor/main.sla
```
