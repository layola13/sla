# 68 String Count

对标 `tsgosa/demos/185_str_join2`（三词中命中 `"b"` 计数 `=1`）。

- `for-of` 用直列 `str_eq` 改写（`for in` 迭代器协议见路线图 Phase 6，暂不碰）；
  字符串断言统一用 `str_eq`（物化口径见 `POTENTIAL_ISSUES.md` #3/#5）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/68_str_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/68_str_count/main.sla
```
