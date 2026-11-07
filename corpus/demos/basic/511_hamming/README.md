# 511 hamming

汉明距离（`85 vs 170 → 8`；510 的应用版）。

- 对标：510 duel_bitcount → 双数同步移位比较（`7 vs 3 → 1`）
- 绕行：`^` 未知算子不用（逐位 `%2` 比较）；长数尾轮另扫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/511_hamming/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/511_hamming/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/511_hamming/main.sla -o /tmp/basic_511 && /tmp/basic_511
# 期望输出：8,1,0
```
