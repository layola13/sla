# 782 bell_rowsum

Bell 二类行和桥 `bsum(9)=21147`，`bsum(10)=115975`（行和恒等续高位）。

- 对标：697_stir2_row7 × 776_bell9 → 两族桥接续高位（行和恒等单锁；785 做三向收束）
- 绕行：`S(10,*)` 十项 `1+511+9330+34105+42525+22827+5880+750+45+1` 手算复核

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/782_bell_rowsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/782_bell_rowsum/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/782_bell_rowsum/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/782_bell_rowsum/main.sla -o /tmp/basic_782 && /tmp/basic_782
# 期望输出：21147,115975
```
