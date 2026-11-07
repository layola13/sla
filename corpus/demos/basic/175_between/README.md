# 175 Between

对标 `tsgosa/demos/1267_between`（`1,0,0`）。

- 与 123 的下标合法性同构：`&&` 连接双边比较（急求值语义见 #21，
  此处两侧皆纯比较，无副作用可观察）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/175_between/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/175_between/main.sla
```
