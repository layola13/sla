# 445 tri fibmod

模斐波那契三向对峙（循环 vs 递归 vs 约化；444 的三向版）。

- 对标：444 duel_fibmod → 递归第三角（`(10,7) → 6` 三向；递归深度 10 安全）
- 绕行：递归基 `n=0/1` 双返回（411 同例）；约化复用 `pisano`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/445_tri_fibmod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/445_tri_fibmod/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/445_tri_fibmod/main.sla -o /tmp/basic_445 && /tmp/basic_445
# 期望输出：6,6,6
```
