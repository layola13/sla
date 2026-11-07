# 356 is prime

试除素数判定（奇数步进；46 的判定版）。

- 对标：46 试除 → `d*d <= n` 奇数步进
- 绕行：无（`1/0` 代替布尔；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/356_is_prime/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/356_is_prime/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/356_is_prime/main.sla -o /tmp/basic_356 && /tmp/basic_356
# 期望输出：1,0,1
```
