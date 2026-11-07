# 57 Iife

对标 `tsgosa/demos/170_iife`（立即调用函数表达式，返回 `42`）。

- 缺口 #13 已修复：闭包字面量可直接调用（callee 契约，双后端一致）；
  先绑定再调用仍为等价写法一并保留（闭包写法参考 `20_closures`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/57_iife/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/57_iife/main.sla
```
