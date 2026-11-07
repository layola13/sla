# 205 Ceil Neg

对标 `tsgosa/demos/1426_ceil_neg`（`ceil(-2.5)=-2`，另断言 `ceil(4.1)=5`）。

- 复用 130 的分支 `ceil`（`as i32` 截断 + 符号分支）；
  130 只断言正数分支，此处覆盖负数分支（向零截断即结果）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/205_ceil_neg/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/205_ceil_neg/main.sla
```
