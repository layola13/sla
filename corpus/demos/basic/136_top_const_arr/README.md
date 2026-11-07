# 136 Top Const Arr

对标 `tsgosa/demos/698_top_const_arr`（`63,20,3`）。

- 顶层 `const` 数组双后端正常（见 133）；顶层 `const` 结构体不可用
  （缺口 #19），顶层 `let` 不可解析（ExpectedDeclaration）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/136_top_const_arr/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/136_top_const_arr/main.sla
```
