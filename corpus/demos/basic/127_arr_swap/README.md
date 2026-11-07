# 127 Arr Swap

对标 `tsgosa/demos/547_swap`（`2,1`）。

- 定长数组作函数形参为调用者可见的可变引用（探针实测），
  此处按实际语义断言。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/127_arr_swap/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/127_arr_swap/main.sla
```
