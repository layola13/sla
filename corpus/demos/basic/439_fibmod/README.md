# 439 fibmod

模斐波那契（`fib(10) mod 7 = 6`；237 的取模版）。

- 对标：237 fib_table → 逐项取模迭代（`fib(100) mod 7 = 3`）
- 绕行：返回 `a=F(n)`（`b` 是下一项，差一坑已修正记档）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/439_fibmod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/439_fibmod/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/439_fibmod/main.sla -o /tmp/basic_439 && /tmp/basic_439
# 期望输出：6,3,5
```
