# 431 prime triplet

素数三元组（`<=20 → 4`；430 的三元版）。

- 对标：430 prime_quad → A/B 两型分别计数（`<=50 → 7`）
- 绕行：`k + 6 <= limit` 封顶；两型独立累加

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/431_prime_triplet/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/431_prime_triplet/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/431_prime_triplet/main.sla -o /tmp/basic_431 && /tmp/basic_431
# 期望输出：4,7
```
