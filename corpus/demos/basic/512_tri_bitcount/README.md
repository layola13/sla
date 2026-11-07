# 512 tri bitcount

位计数三向对峙（消位 vs 移位 vs 递归；510 的三向版）。

- 对标：510 duel_bitcount → 递归第三角（`255 → 8` 三向）
- 绕行：递归基 `n==0` 返 0（411 同例）；`&` 加括号

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/512_tri_bitcount/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/512_tri_bitcount/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/512_tri_bitcount/main.sla -o /tmp/basic_512 && /tmp/basic_512
# 期望输出：8,8,8
```
