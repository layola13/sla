# 627 motz_seq

Motzkin 序列前 6 项 `1,1,2,4,9,21`（前缀和 `mozseq(5)=38`；循环求和）。

- 对标：626_motzkin → 单值版互补（同式序列复算；和 main 直打）
- 绕行：`moz(0)` 空表端点直接返回；`mozseq(3)=8` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/627_motz_seq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/627_motz_seq/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/627_motz_seq/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/627_motz_seq/main.sla -o /tmp/basic_627 && /tmp/basic_627
# 期望输出：1,1,38
```
