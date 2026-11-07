# 360 cube sum

立方和循环版（`5 → 225`；359 的立方版）。

- 对标：359 sq_sum → 立方项累加（另验 `3 → 36`）
- 绕行：无（纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/360_cube_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/360_cube_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/360_cube_sum/main.sla -o /tmp/basic_360 && /tmp/basic_360
# 期望输出：225,36
```
