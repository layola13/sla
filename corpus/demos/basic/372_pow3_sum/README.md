# 372 pow3 sum

3 的幂和（`n=4 → 40`；352 的 3 进制版）。

- 对标：352 geom_sum → 公比 3 步进累加
- 绕行：无（纯 `int` while 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/372_pow3_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/372_pow3_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/372_pow3_sum/main.sla -o /tmp/basic_372 && /tmp/basic_372
# 期望输出：40,13,1
```
