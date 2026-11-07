# 173 Xor Swap

对标 `tsgosa/demos/1251_xor_swap`（`9,5`）。

- SLA 无 `^=`（无词法 token，见 119），用三遍普通 `^` 赋值表达；
  与 127 的临时变量交换机制互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/173_xor_swap/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/173_xor_swap/main.sla
```
