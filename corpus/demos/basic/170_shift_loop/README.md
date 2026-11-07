# 170 Shift Loop

对标 `tsgosa/demos/1234_shift_loop`（`16`）。

- 与 52 的乘法翻倍同值不同机制：此处 `x = x << 1`（`<<=` 无 token，
  用显式赋值，见 119）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/170_shift_loop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/170_shift_loop/main.sla
```
