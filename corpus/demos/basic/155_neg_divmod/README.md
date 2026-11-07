# 155 Neg Divmod

对标 `tsgosa/demos/383_div_guard` 的负数延伸（`-1,1,-2`）。

- 383 只覆盖正数除余；此处锁定负数语义：向零截断（C 式），
  余数符号随被除数（与 TS 一致，双后端 + exe 核对）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/155_neg_divmod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/155_neg_divmod/main.sla
```
