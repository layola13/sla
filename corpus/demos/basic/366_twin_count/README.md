# 366 twin count

孪生素数计数（`(3,5),(5,7),(11,13),(17,19)`；356 的对版）。

- 对标：356 is_prime → 双判定计数
- 绕行：无（纯 `int` 循环；嵌套 `if` 代替 `&&` 短路）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/366_twin_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/366_twin_count/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/366_twin_count/main.sla -o /tmp/basic_366 && /tmp/basic_366
# 期望输出：4,2
```
