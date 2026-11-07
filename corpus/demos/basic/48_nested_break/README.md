# 48 Nested Break

对标 `tsgosa/demos/157_label_nested`（`continue outer`，`t=3`）。

SLA 暂无 label 语法（见 POTENTIAL_ISSUES #9），`continue outer` 改写为内层 `break`，
在此用例中等价（`j==1` 后本轮内层剩余迭代均无计数）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/48_nested_break/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/48_nested_break/main.sla
```
