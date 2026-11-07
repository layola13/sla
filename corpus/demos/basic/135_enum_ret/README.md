# 135 Enum Ret

对标 `tsgosa/demos/575_enumret`（`1,0`）。

- 枚举用 `enum E { A, B }` + `E::B` 具限（见 75），`match` 消费；
  裸 `E.B` 不可解析，用 `match` 转整数断言。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/135_enum_ret/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/135_enum_ret/main.sla
```
