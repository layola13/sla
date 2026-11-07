# 414 tri dsum

数位和三向对峙（循环 vs 递归 vs 迭代求根；411 的三向版）。

- 对标：411 duel_dsum → 求根第三角（`12345 → 15=15`，根 `6`）
- 绕行：无（复用循环体；递归基 `n < 10`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/414_tri_dsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/414_tri_dsum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/414_tri_dsum/main.sla -o /tmp/basic_414 && /tmp/basic_414
# 期望输出：15,15,6
```
