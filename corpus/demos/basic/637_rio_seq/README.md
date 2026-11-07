# 637 rio_seq

Riordan 序列前 7 项 `1,0,1,1,3,6,15`（前缀和 `rioseq(6)=27`；循环求和）。

- 对标：636_riordan → 单值版互补（同式序列复算；和 main 直打）
- 绕行：`R(1)=0` 零值项和中自然归零；`rioseq(4)=6` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/637_rio_seq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/637_rio_seq/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/637_rio_seq/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/637_rio_seq/main.sla -o /tmp/basic_637 && /tmp/basic_637
# 期望输出：1,0,27
```
