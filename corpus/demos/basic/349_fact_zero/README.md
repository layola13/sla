# 349 fact zero

阶乘末尾零（`tz(n!) = n/5 + n/25 + …`；125 的数位应用版）。

- 对标：125 fact → 末尾零计数
- 绕行：无（纯 `int` 除法循环；`d <= n` 乘法步进）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/349_fact_zero/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/349_fact_zero/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/349_fact_zero/main.sla -o /tmp/basic_349 && /tmp/basic_349
# 期望输出：2,6,1
```
