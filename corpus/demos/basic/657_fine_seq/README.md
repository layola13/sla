# 657 fine_seq

Fine 序列前 7 项 `1,0,1,2,6,18,57`（前缀和 `fseq(6)=85`；循环求和）。

- 对标：656_fine → 单值版互补（同式序列复算；和 main 直打）
- 绕行：`F(1)=0` 零值项和中自然归零；`fseq(4)=10` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/657_fine_seq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/657_fine_seq/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/657_fine_seq/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/657_fine_seq/main.sla -o /tmp/basic_657 && /tmp/basic_657
# 期望输出：1,0,85
```
