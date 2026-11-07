# 47 Default Args

对标 `tsgosa/demos/150_multi_default`（`f(a=1,b=2,c=3)`）。

SLA 暂不支持默认参数值（`fn f(a: i32 = 1)` 直接 parse 失败，见 POTENTIAL_ISSUES #8），
本 demo 用 `f_default()`（零参，返回 `1+2+3`）+ `f_full(a,b,c)`（全参）表达同一调用语义。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/47_default_args/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/47_default_args/main.sla
```
