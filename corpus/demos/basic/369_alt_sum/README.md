# 369 alt sum

交错和（`10 → -5`；359 的符号版，375 做三向）。

- 对标：359 sq_sum → 奇加偶减（双 `if` 避 `else` 语义坑）
- 绕行：无（纯 `int` 循环；`% 2` 判奇偶）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/369_alt_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/369_alt_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/369_alt_sum/main.sla -o /tmp/basic_369 && /tmp/basic_369
# 期望输出：-5,3,1
```
