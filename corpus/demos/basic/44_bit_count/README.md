# 44 Bit Count

对标 `tsgosa/demos/148_bit_count`。

覆盖位运算 `&`（取位）+ `>>`（右移）。注意形参不可再赋值，
此处用 `let x = mut_x;` 转局部可写绑定（SLA 仿射语义）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/44_bit_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/44_bit_count/main.sla
```
