# 418 weak goldbach

弱哥德巴赫分拆计数（`15 → 2`；380 的奇三元版）。

- 对标：380 goldbach → 三重循环无序去重（`p≤q≤r`，偶素数 2 排除）
- 绕行：`r >= q` 保无序唯一；非正余数由素性函数兜底判 0；
  内层 `while` 后不加 `;`（386 同例，三重循环亦如此）
- 绕行：`r >= q` 保无序唯一；非正余数由素性函数兜底判 0

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/418_weak_goldbach/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/418_weak_goldbach/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/418_weak_goldbach/main.sla -o /tmp/basic_418 && /tmp/basic_418
# 期望输出：1,1,2
```
