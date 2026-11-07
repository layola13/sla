# 540 oddgrow

奇部数分拆增长段（`p_odd(8..10)=6,8,10`；527 的高位延续，541 做 Euler 对峙）。

- 对标：539_qgrow → 互异版互补（双 DP 独立算出同值，Euler 逐点成立）
- 绕行：539 手算修正值直接采用（`8→8`、`9→8`、`10→10` 一次绿，反向确证 DP）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/540_oddgrow/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/540_oddgrow/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/540_oddgrow/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/540_oddgrow/main.sla -o /tmp/basic_540 && /tmp/basic_540
# 期望输出：6,8,10
```
