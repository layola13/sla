# 432 duel peak

峰值双实现对峙（循环 vs 递归；411 的冰雹版）。

- 对标：411 duel_dsum → 峰值跟踪双边锁定（`27 → 9232=9232`）
- 绕行：递归携带 `peak` 累积器（412 同例；草稿函数已删）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/432_duel_peak/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/432_duel_peak/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/432_duel_peak/main.sla -o /tmp/basic_432 && /tmp/basic_432
# 期望输出：9232,9232
```
