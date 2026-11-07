# 95 String Iter

对标 `tsgosa/demos/11_string_array_iter`（累计 `6`，映射和 `6`）。

- `forEach/map` 用直列 `len()` 改写（`for in` 迭代器协议见 Phase 6，暂不碰；
  变量 `len` 路径见缺口 #10，写法见 `68/69`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/95_str_iter/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/95_str_iter/main.sla
```
