# 380 goldbach

Goldbach 分拆计数（`10 → 2`，`12 → 1`，`20 → 2`；366 的和版）。

- 对标：366 twin_count → `p + (n-p)` 双素数计数（`p <= n/2` 去重）
- 绕行：无（嵌套 `if` 代替 `&&`；复用 `is_prime`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/380_goldbach/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/380_goldbach/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/380_goldbach/main.sla -o /tmp/basic_380 && /tmp/basic_380
# 期望输出：2,1,2
```
