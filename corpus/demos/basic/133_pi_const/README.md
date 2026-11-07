# 133 Pi Const

对标 `tsgosa/demos/574_piconst`（`1,0`）。

- SLA 支持顶层 `const`（探针已验证）；`PI` 取 3.14159 近似。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/133_pi_const/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/133_pi_const/main.sla
```
