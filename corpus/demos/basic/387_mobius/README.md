# 387 mobius

莫比乌斯函数 μ（`μ(30)=-1`，`μ(12)=0`；386 的符号版）。

- 对标：386 omega → 平方因子即归零（`0 - 1` 表负数，避一元负号坑）
- 绕行：无（试除中途查重因子；奇偶定符号）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/387_mobius/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/387_mobius/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/387_mobius/main.sla -o /tmp/basic_387 && /tmp/basic_387
# 期望输出：1,1,-1,0
```
