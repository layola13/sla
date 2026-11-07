# 430 prime quad

素数四元组（`<=50 → 2`；366 的四元版）。

- 对标：366 twin_count → 四点全素判定（5/11 两组）
- 绕行：`k + 8 <= limit` 封顶；早返回四段式

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/430_prime_quad/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/430_prime_quad/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/430_prime_quad/main.sla -o /tmp/basic_430 && /tmp/basic_430
# 期望输出：1,2
```
