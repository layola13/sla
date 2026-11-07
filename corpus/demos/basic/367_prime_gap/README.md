# 367 prime gap

素数最大间隙（`<=30 → 6`；358 的间隙版）。

- 对标：358 prime_sum → 前驱差值跟踪（`23→29` 间隙 6）
- 绕行：无（纯 `int` 循环复用 `is_prime`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/367_prime_gap/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/367_prime_gap/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/367_prime_gap/main.sla -o /tmp/basic_367 && /tmp/basic_367
# 期望输出：6,2
```
