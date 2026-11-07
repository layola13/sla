# 239 F64 Arith

f64 减乘除（278 只覆盖加法，此处补全四则：`5.0/10.0/3.0`）。

- 裸 f64 不直打（打印截断见 #29），以比较转 int 旗输出。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/239_f64_arith/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/239_f64_arith/main.sla
```
