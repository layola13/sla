# 440 powmod sum

几何级数模和（`(2,10,100) → 23`；352 的取模版）。

- 对标：352 geom_sum → 逐项取模累加（`(3,5,100) → 21`）
- 绕行：无（`s + t` 先加后模防溢出；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/440_powmod_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/440_powmod_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/440_powmod_sum/main.sla -o /tmp/basic_440 && /tmp/basic_440
# 期望输出：23,21,1
```
