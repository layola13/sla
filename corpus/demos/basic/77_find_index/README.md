# 77 Find Index

对标 `tsgosa/demos/108_find_index`（首个 `>10` 元素的下标 `=1`）。

- 高阶 `findIndex` 用显式循环表达，谓词用先绑定再调用的闭包（写法见 `20/57`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/77_find_index/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/77_find_index/main.sla
```
