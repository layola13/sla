# 206 Iife Nest

对标 `tsgosa/demos/864_top_iife_nest`（`((x)=>((y)=>x+y)(10))(5)=15`）。

- 非捕获嵌套字面量直接调用双后端正常（#13 口径）；
  捕获型直接调用见缺口 #26。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/206_iife_nest/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/206_iife_nest/main.sla
```
