# 92 And Or

对标 `tsgosa/demos/197_and_or`（假，真，真）。

- TS 用整数真值，SLA 中 `&&/||/!` 只接受布尔，故用比较式改写（`&&` 口径见 `14`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/92_and_or/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/92_and_or/main.sla
```
