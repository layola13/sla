# 11 Struct

对标 `tsgosa/demos/13_struct` + `19_destructure`（对象部分）。

`struct Pt { x, y }` + `Pt { x: 3, y: 4 }` 构造 + `p.x` 读写。
数组解构 `[x, y] = [7, 8]` 在 SLA 中用定长数组索引表达（见 08_arrays）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/11_struct/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/11_struct/main.sla
```
