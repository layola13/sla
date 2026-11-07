# 408 persist

加法持久性（`199 → 3`；406 的迭代版）。

- 对标：406 dsum → `m >= 10` 反复折叠计数（`39 → 2`，一位数 0 步）
- 绕行：无（复用 `dsum`；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/408_persist/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/408_persist/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/408_persist/main.sla -o /tmp/basic_408 && /tmp/basic_408
# 期望输出：3,2,0
```
