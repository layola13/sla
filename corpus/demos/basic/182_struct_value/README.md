# 182 Struct Value

对标 `tsgosa/demos/476_field_compound`（函数内 `11`，主调方 `10`）。

- 定长数组作形参调用者可见（127），结构体作形参则为值语义：
  函数内 `+=` 只改本地副本（双后端 + exe 核对）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/182_struct_value/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/182_struct_value/main.sla
```
