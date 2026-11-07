# 188 F64 Pick

对标 `tsgosa/demos/475_f64_mixed_ternary`（`true,true`）。

- 与 185 的整数嵌套三元互补：此处为 f64 条件 + f64 分支
  （探针已验证三线；混合传参用 f64 字面量）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/188_f64_pick/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/188_f64_pick/main.sla
```
