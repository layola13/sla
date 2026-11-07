# 03 If Else

对标 `tsgosa/demos/03_if_else`（`max(a, b)`）。

SLA 的 `if/else` 是表达式（`if c { a } else { b }`），编译器自动处理分支汇合的
Phi 状态（见语言规范 §3.1），用户无需手写 `!reg`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/03_if_else/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/03_if_else/main.sla
```
