# 399 phi mult

phi 乘性（`φ(12)=φ(4)φ(3)=4`；336 的乘性版）。

- 对标：336 euler_phi → 互素分解乘积（`φ(10)=4` 另断言）
- 绕行：`phi(1)=1` 约定复用（382 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/399_phi_mult/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/399_phi_mult/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/399_phi_mult/main.sla -o /tmp/basic_399 && /tmp/basic_399
# 期望输出：4,4,4
```
