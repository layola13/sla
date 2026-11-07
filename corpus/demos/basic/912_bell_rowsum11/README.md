# 912 bell_rowsum11

Bell 二类行和桥续高位 `bsum(11)=678570`，`bsum(12)=4213597`（行和恒等续高位）。

- 对标：906_bell11 × 782_bell_rowsum → 两族桥接续高位（915 做三向收束）
- 绕行：无（行和桥双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/912_bell_rowsum11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/912_bell_rowsum11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/912_bell_rowsum11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/912_bell_rowsum11/main.sla -o /tmp/basic_912 && /tmp/basic_912
# 期望输出：678570,4213597,115975
```
