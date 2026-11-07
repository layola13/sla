# 703 stir2_bell

Stirling 二类 Bell 桥 `bell(n+1)=sum C(n,k)·bell(k)`（`bell(6)=203`，`bell(7)=877`，`bell(8)=4140`）。

- 对标：697_stir2_row7 × Bell 族 → 两族桥接（行和恒等单锁；705 做三向收束）
- 绕行：`bell(5)=52` 小值守卫；递归深度止 `8`（`2^8` 扇出内，两后端秒级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/703_stir2_bell/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/703_stir2_bell/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/703_stir2_bell/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/703_stir2_bell/main.sla -o /tmp/basic_703 && /tmp/basic_703
# 期望输出：203,877,4140
```
