# 814 eulb_fact67

Eulerian-B 行和桥续高位：`ebsum(6)=46080`，`ebsum(7)=645120`（`p2f` 陪锁）。

- 对标：807_eulb_row6 × 684_eulb_fact → 两族桥接续高位（行和恒等单锁；815 做三向收束）
- 绕行：`p2f(7)=128·5040=645120` 同值陪锁（684 体例续高位；`2^n` 连乘避 `>>`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/814_eulb_fact67/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/814_eulb_fact67/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/814_eulb_fact67/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/814_eulb_fact67/main.sla -o /tmp/basic_814 && /tmp/basic_814
# 期望输出：46080,46080,645120,645120
```
