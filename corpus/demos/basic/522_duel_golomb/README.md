# 522 duel golomb

Golomb 双实现对峙（纯递归 vs 外层迭代；453 的自指版）。

- 对标：453 duel_trib → 求值序不同锁定（`G(9)=5=5`）
- 绕行：`n==1` 单基双边；迭代版 `i<=n` 含末项（初版 `<n` 差一，记档）；
  死变量 `a` 已删（`b` 直赋）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/522_duel_golomb/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/522_duel_golomb/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/522_duel_golomb/main.sla -o /tmp/basic_522 && /tmp/basic_522
# 期望输出：5,5
```
