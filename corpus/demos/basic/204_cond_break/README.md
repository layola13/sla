# 204 Cond Break

对标 `tsgosa/demos/1268_while_break`（`1+2+3+5+6=17`）。

- 条件 `while` + `continue` + `break` 三者同环；
  与 32（`while true` 版）、147（无 `break` 版）互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/204_cond_break/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/204_cond_break/main.sla
```
