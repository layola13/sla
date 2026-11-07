# 12 Impl Counter

对标 `tsgosa/demos/14_class`（`class Counter { inc() }`）。

SLA 没有 `class`，用 `struct + impl` 表达。注意 SLA 方法是消费式（`inc(self)` 返回新值），
调用侧需 `c = c.inc();` 而非 `c.inc();`（线性/仿射语义，见语言规范 §2）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/12_impl_counter/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/12_impl_counter/main.sla
```
