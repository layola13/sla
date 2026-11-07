# 179 Compare Chain

对标 `tsgosa/demos/1258_compare_chain`（`1,1`）。

- 与 175 的 `&&` 区间判定同构：此处为传递链（急求值见 #21，
  两侧纯比较无副作用）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/179_compare_chain/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/179_compare_chain/main.sla
```
