# 357 nth prime

第 n 个素数（`nth(10)=29`；356 的计数版）。

- 对标：356 判定 → 有界扫描计数（上界 1000，避无界循环）
- 绕行：无（纯 `int` 循环复用 `is_prime`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/357_nth_prime/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/357_nth_prime/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/357_nth_prime/main.sla -o /tmp/basic_357 && /tmp/basic_357
# 期望输出：29,13
```
