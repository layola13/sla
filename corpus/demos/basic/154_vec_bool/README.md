# 154 Vec Bool

对标 `tsgosa/demos/388_boolkw` 的 Vec 版（`2,true,false`）。

- 124 覆盖定长 `bool` 数组，此处换 `Vec<bool>`（与 59 的换 Vec 版先例一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/154_vec_bool/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/154_vec_bool/main.sla
```
