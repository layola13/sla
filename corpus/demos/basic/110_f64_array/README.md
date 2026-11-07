# 110 F64 Array

对标 `tsgosa/demos/315_float_arr`（长 4，和 4.5）。

- 定长数组元素类型一致即放行（#1：错配才拦截）；函数返回数组值语义
  另见缺口 #17，此处只做局部存取。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/110_f64_array/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/110_f64_array/main.sla
```
