# 713 lah_edge

Lah 对角边 `L(n,n-1)=n·(n-1)`：`L(6,5)=30`，`L(7,6)=42`，`L(8,7)=56`（边值恒等）。

- 对标：706_lah7 → 内格版互补（边值恒等复算；715 做三向收束）
- 绕行：`eprod(8)=56` 同值陪锁（602 边值体例复用；`L(8,7)=L(7,6)+14·L(7,7)=42+14=56` 单步守卫）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/713_lah_edge/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/713_lah_edge/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/713_lah_edge/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/713_lah_edge/main.sla -o /tmp/basic_713 && /tmp/basic_713
# 期望输出：30,42,56
```
