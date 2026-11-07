# 158 Step3 Down

对标 `tsgosa/demos/1048_for_dec`（`22`）。

- SLA `for` 为无步长整数区间（`.step()` 不可解析），步长循环用
  `while` 表达（与 50/61 的 while 口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/158_step3_down/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/158_step3_down/main.sla
```
