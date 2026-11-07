# 362 harm bound

调和和整数缩放版（`100*H_4 = 208`；141 的倒数版）。

- 对标：141 sum_sq → 倒数项按项整除累加（避 #29 f64 直打坑，全 `int`）
- 绕行：无（`100/i` 逐项整除；`4 → 100+50+33+25`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/362_harm_bound/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/362_harm_bound/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/362_harm_bound/main.sla -o /tmp/basic_362 && /tmp/basic_362
# 期望输出：208,150
```
