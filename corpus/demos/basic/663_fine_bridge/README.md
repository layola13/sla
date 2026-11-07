# 663 fine_bridge

Fine 桥序 `F(n)=(Catalan(n)-F(n-1))/2`（`F(4)=6`，`F(5)=18`，`F(6)=57`）。

- 对标：656_fine × 351_catalan → 两族桥接（差序单锁；665 做三向收束）
- 绕行：`C(6)-F(5)=132-18=114` 差偶数恒成立（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/663_fine_bridge/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/663_fine_bridge/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/663_fine_bridge/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/663_fine_bridge/main.sla -o /tmp/basic_663 && /tmp/basic_663
# 期望输出：6,18,57
```
