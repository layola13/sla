# 194 F64 Param

对标 `tsgosa/demos/1011_arr_param_twice` 的定长改写（`true`）。

- 原用例两次传不同长度数组（SLA 定长类型下不可表达，见 #1），
  此处取单次等长传参：f64 数组形参求和（与 116 的 i32 版互补）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/194_f64_param/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/194_f64_param/main.sla
```
