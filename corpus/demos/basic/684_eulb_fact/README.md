# 684 eulb_fact

Eulerian-B 行和即公式：`ebsum(3)=48`，`ebsum(4)=384`，`ebsum(5)=3840`（`2^n·n!` 桥）。

- 对标：677_eulb_row × 230_fact_iter → 两族桥接（行和恒等单锁；685 做三向收束）
- 绕行：`p2f(5)=32·120=3840` 同值陪锁（230 迭代体例复用；`2^n` 连乘避 `>>`，515 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/684_eulb_fact/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/684_eulb_fact/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/684_eulb_fact/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/684_eulb_fact/main.sla -o /tmp/basic_684 && /tmp/basic_684
# 期望输出：48,384,3840
```
