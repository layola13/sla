# 352 geom sum

等比求和（公比 2；330 等差版的等比版）。

- 对标：330 arith_seq → 乘法步进累加
- 绕行：无（纯 `int` while 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/352_geom_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/352_geom_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/352_geom_sum/main.sla -o /tmp/basic_352 && /tmp/basic_352
# 期望输出：63,15
```
