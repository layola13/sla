# 777 bell_seq

Bell 序列前 10 项和 `26443`（`B(8)=4140`，`B(9)=21147`；循环求和）。

- 对标：776_bell9 → 单值版互补（同式序列复算；和 main 直打）
- 绕行：十项和 `5296+21147=26443` 手算复核；`bseq(5)=76` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/777_bell_seq/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/777_bell_seq/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/777_bell_seq/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/777_bell_seq/main.sla -o /tmp/basic_777 && /tmp/basic_777
# 期望输出：4140,21147,26443
```
