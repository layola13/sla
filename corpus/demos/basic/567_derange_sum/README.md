# 567 derange_sum

错排容斥和式 `!n=Σ_{k=0}^{n}(-1)^k·n!/k!`（`n!/k!` 取 `(n-k)` 项连乘：`!3..!5=2,9,44`）。

- 对标：566_derange → 递推版互补（568 做双实现对峙）
- 绕行：初版下降阶乘方向写反（`k` 项 vs `n-k` 项，panic 56701 定位修正）；符号 `0-1` 技巧（375 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/567_derange_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/567_derange_sum/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/567_derange_sum/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/567_derange_sum/main.sla -o /tmp/basic_567 && /tmp/basic_567
# 期望输出：2,9,44
```
