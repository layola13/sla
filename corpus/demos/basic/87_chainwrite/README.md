# 87 Chainwrite

对标 `tsgosa/demos/113_chainwrite`（`10+2=12`）。

- 与 `79`（嵌套读）互补，此处覆盖嵌套写路径（`q.p.x = 10` 探针已验证）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/87_chainwrite/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/87_chainwrite/main.sla
```
