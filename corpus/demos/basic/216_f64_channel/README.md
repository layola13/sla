# 216 F64 Channel

对标 `tsgosa/demos/278_f64_channel`（`1,1,1`）。

- f64 变量 + 加法 + 函数返回 + 相等判定；裸 f64 不直打
  （打印截断见 #29），以比较转 int 旗输出。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/216_f64_channel/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/216_f64_channel/main.sla
```
