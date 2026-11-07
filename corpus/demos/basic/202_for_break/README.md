# 202 For Break

对标 `tsgosa/demos/1270_for_break`（`0+1+2+3+4=10`）。

- SLA `for` 为无步长整数区间（见 158），C 式三段头用区间改写。
- 与 32（`while` 版早停）、48（嵌套版）互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/202_for_break/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/202_for_break/main.sla
```
