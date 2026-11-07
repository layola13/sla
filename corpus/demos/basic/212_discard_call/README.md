# 212 Discard Call

对标 `tsgosa/demos/672_void_i32`（只打印 `1`，`three()` 丢弃）。

- i32 返回值作语句丢弃；141（`push` 作语句）已背书该写法，
  此处为用户函数版。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/212_discard_call/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/212_discard_call/main.sla
```
