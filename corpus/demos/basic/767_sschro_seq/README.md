# 767 sschro_seq

小 Schröder 序列前 7 项 `1,1,3,11,45,197,903`（前缀和 `sseq(6)=1161`；循环求和）。

- 对标：766_sschro → 单值版互补（同式序列复算；和 main 直打）
- 绕行：`sseq(4)=61` 小值守卫；`s(0)=s(1)=1` 双端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/767_sschro_seq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/767_sschro_seq/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/767_sschro_seq/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/767_sschro_seq/main.sla -o /tmp/basic_767 && /tmp/basic_767
# 期望输出：1,1,1161
```
