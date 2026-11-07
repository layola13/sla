# 465 tri delannoy

Delannoy 三向对峙（格路 vs 二项和 vs 中央递推；464 的三向版）。

- 对标：464 duel_delannoy → 中央递推第三角（`63` 三向；除法精确）
- 绕行：中央递推 `n=0/1` 双基（除数 `n` 恒整除；先乘后除）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/465_tri_delannoy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/465_tri_delannoy/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/465_tri_delannoy/main.sla -o /tmp/basic_465 && /tmp/basic_465
# 期望输出：63,63,63
```
