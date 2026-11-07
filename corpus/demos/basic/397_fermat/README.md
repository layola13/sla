# 397 fermat

费马素性测试（`341` 伪素数反例记档；356 的概率版）。

- 对标：356 is_prime → `2^(n-1) mod n`（`7 → 1`，`9 → 0`，`341 → 1` 但试除判合）
- 绕行：无（复用平方乘 `powmod`；伪素数属数论事实非缺口）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/397_fermat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/397_fermat/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/397_fermat/main.sla -o /tmp/basic_397 && /tmp/basic_397
# 期望输出：1,0,1,0
```
