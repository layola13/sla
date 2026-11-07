# 448 tribonacci

Tribonacci 数（`T(10)=81`；237 的三阶版）。

- 对标：237 fib_table → 三窗口滑动（`T(7)=13`）
- 绕行：`n=0/1/2` 三提前返回（三初值，442 类推）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/448_tribonacci/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/448_tribonacci/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/448_tribonacci/main.sla -o /tmp/basic_448 && /tmp/basic_448
# 期望输出：81,13,1
```
