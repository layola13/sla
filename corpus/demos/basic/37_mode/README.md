# 37 Mode

对标 `tsgosa/demos/146_mode`。

双重 `for` 计数（众数 `3`，频次 `3`）。嵌套循环的变量清理语义
见 `tests/test_unit_loop_body_local_cleanup.sla`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/37_mode/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/37_mode/main.sla
```
