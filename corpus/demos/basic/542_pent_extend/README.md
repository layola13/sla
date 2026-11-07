# 542 pent_extend

五边形数定理高位延伸（`p(8..10)=22,30,42`；528 的高位延续，543 做 DP 对峙）。

- 对标：528_pent → 低位版互补（同式高位复算：`p(8)=15+11-3-1`）
- 绕行：`0 - 1` 表负号（528 同例）；536 背包值反向确证（22/30/42 一致）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/542_pent_extend/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/542_pent_extend/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/542_pent_extend/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/542_pent_extend/main.sla -o /tmp/basic_542 && /tmp/basic_542
# 期望输出：22,30,42
```
