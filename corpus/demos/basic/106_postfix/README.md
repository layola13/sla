# 106 Postfix

对标 `tsgosa/demos/268_postfix`（`0,1,1,0,10,1`）。

- SLA 尚无 `++`/`--`（parser 直接拒绝），此处用显式 `i = i + 1` /
  `i = i - 1` 表达同一语义（含数组索引步进）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/106_postfix/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/106_postfix/main.sla
```
