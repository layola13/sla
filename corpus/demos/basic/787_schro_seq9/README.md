# 787 schro_seq9

大 Schröder 序列前 10 项和 `258563`（`S(8)=41586`，`S(9)=206098`；循环求和）。

- 对标：786_schro9 → 单值版互补（同式序列复算；和 main 直打）
- 绕行：十项和 `52465+206098=258563` 手算复核；`schseq(6)=2321` 低位守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/787_schro_seq9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/787_schro_seq9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/787_schro_seq9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/787_schro_seq9/main.sla -o /tmp/basic_787 && /tmp/basic_787
# 期望输出：41586,206098,258563
```
