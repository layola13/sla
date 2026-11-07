# 120 Recurse

对标 `tsgosa/demos/377_funexpr_rec`（`fact(5)=120`，另加 `fib(10)=55`）。

- 原用例为函数表达式自引用，SLA 用普通具名函数递归表达同一语义
  （与 18/49 的迭代口径互补，探针已验证双后端）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/120_recurse/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/120_recurse/main.sla
```
