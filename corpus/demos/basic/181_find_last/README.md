# 181 Find Last

对标 `tsgosa/demos/430_find_last`（`2,-1`）。

- SLA 无 `findLast`（闭包作参数拒识，见 385），用倒序循环表达
  （与 116/140 的正向查找方向互补；`len()` 先 `as i32`，见 160）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/181_find_last/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/181_find_last/main.sla
```
