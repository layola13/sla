# 693 stir1_edge

Stirling 一类次对角 `c(n,n-1)=C(n,2)`：`c(6,5)=15`，`c(7,6)=21`，`c(8,7)=28`（三角数恒等）。

- 对标：686_stir17 → 内格版互补（边值恒等复算；695 做三向收束）
- 绕行：`binom(8,2)=28` 同值陪锁（546 二项体例复用；`21+7=28` 单步守卫）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/693_stir1_edge/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/693_stir1_edge/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/693_stir1_edge/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/693_stir1_edge/main.sla -o /tmp/basic_693 && /tmp/basic_693
# 期望输出：15,21,28
```
