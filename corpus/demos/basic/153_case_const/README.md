# 153 Case Const

对标 `tsgosa/demos/948_case_ident`（`1,0`）。

- 顶层 `const` 作 case 臂双后端正常（133 证常量可用；注意顶层仅容 `const`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/153_case_const/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/153_case_const/main.sla
```
