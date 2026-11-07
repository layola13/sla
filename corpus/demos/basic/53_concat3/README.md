# 53 Concat3

对标 `tsgosa/demos/161_concat3`（`[1].concat([2], [3, 4])`，`len/first/last = 4/1/4`）。

- SLA 无 `concat` 方法，按 `39` 的 `Vec::push` 写法逐个压入。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/53_concat3/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/53_concat3/main.sla
```
