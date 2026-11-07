# 361 fib even sum

偶斐波那契和（`<=50 → 44`；237 的偶项版）。

- 对标：237 fib 表 → 偶项过滤累加（`2+8+34`）
- 绕行：无（纯 `int` 迭代；`% 2` 判定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/361_fib_even_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/361_fib_even_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/361_fib_even_sum/main.sla -o /tmp/basic_361 && /tmp/basic_361
# 期望输出：10,44
```
