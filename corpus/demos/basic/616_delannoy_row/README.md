# 616 delannoy_row

Delannoy 第 5 行 `1,11,61,231,681,1683`（行和 `2668`；循环求和）。

- 对标：611_delannoy_tab → 建表版互补（同式高位行复算；行和 main 直打）
- 绕行：对称复核 `D(5,2)=D(2,5)=61`（行列双算一致）；`D(5,0)=1` 左端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/616_delannoy_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/616_delannoy_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/616_delannoy_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/616_delannoy_row/main.sla -o /tmp/basic_616 && /tmp/basic_616
# 期望输出：11,61,1683
```
