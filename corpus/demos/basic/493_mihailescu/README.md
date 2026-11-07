# 493 mihailescu

Mihăilescu 特例搜索（`≤20 → 1` 组 `(9,8)`；397 的幂差版）。

- 对标：397 fermat → 平方立方差 ±1 枚举（定理实例，属数论事实）
- 绕行：`0 - 1` 表负差（387 同例）；输出用差值不用布尔直打

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/493_mihailescu/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/493_mihailescu/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/493_mihailescu/main.sla -o /tmp/basic_493 && /tmp/basic_493
# 期望输出：1,0
```
