# 510 duel bitcount

位计数双实现对峙（消位法 vs 移位法；411 的位版）。

- 对标：411 duel_dsum → 双路径锁定（`255 → 8=8`）
- 绕行：`&` 加括号（145 口径）；移位用 `/2`（避 `>>`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/510_duel_bitcount/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/510_duel_bitcount/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/510_duel_bitcount/main.sla -o /tmp/basic_510 && /tmp/basic_510
# 期望输出：8,8
```
