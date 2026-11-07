# 85 Rest First

对标 `tsgosa/demos/154_rest_first`（`10+3=13`）。

- SLA 暂无剩余参数语法，用定长 `[int; 3]` 形参表达同一语义（类型口径见缺口 #1）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/85_rest/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/85_rest/main.sla
```
