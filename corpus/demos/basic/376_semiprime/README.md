# 376 semiprime

半素数判定（恰两个素因子计重数；356 的合数版）。

- 对标：356 is_prime → 试除计数因子（`6 → 1`，`12 → 0`）
- 绕行：无（嵌套 while 试除；`m > 1` 收尾余因子）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/376_semiprime/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/376_semiprime/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/376_semiprime/main.sla -o /tmp/basic_376 && /tmp/basic_376
# 期望输出：1,1,0,1
```
