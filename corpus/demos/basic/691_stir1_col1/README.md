# 691 stir1_col1

Stirling 一类首列即阶乘：`c(6,1)=120`，`c(7,1)=720`，`c(8,1)=5040`（首列恒等单锁）。

- 对标：686_stir17 → 内格版互补（首列恒等复算；692 做首列对峙）
- 绕行：`c(n,1)=(n-1)!` 三值连验（230 迭代体例复用）；`c(5,1)=24` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/691_stir1_col1/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/691_stir1_col1/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/691_stir1_col1/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/691_stir1_col1/main.sla -o /tmp/basic_691 && /tmp/basic_691
# 期望输出：120,720,5040
```
