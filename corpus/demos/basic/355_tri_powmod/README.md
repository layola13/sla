# 355 tri powmod

模幂三向对峙（连乘 vs 平方乘；328 的三向版）。

- 对标：328 powmod → 双实现一致性锁定（`3^5 mod 7 = 5`，`2^10 mod 100 = 24`）
- 绕行：无（纯 `int` 循环；`e % 2` 分支）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/355_tri_powmod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/355_tri_powmod/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/355_tri_powmod/main.sla -o /tmp/basic_355 && /tmp/basic_355
# 期望输出：5,5,24
```
