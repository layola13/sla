# 40 Do Sum

对标 `tsgosa/demos/158_do_sum`（`do { } while` 语义：至少执行一次）。

SLA 暂无 `do-while` 语法，用 `while true + break` 表达同一语义
（与 `32_while_break` 同构），`do_sum(5) = 0+1+2+3+4 = 10`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/40_do_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/40_do_sum/main.sla
```
