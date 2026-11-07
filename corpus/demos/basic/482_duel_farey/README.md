# 482 duel farey

Farey 双实现对峙（φ 求和 vs 互素对枚举；374 的分数版）。

- 对标：374 duel_primecount → 公式与暴力锁定（`11=11`）
- 绕行：`gcd(0,1)=1` 计入端点（`0/1`），`gcd(0,b>1)` 自然排除

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/482_duel_farey/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/482_duel_farey/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/482_duel_farey/main.sla -o /tmp/basic_482 && /tmp/basic_482
# 期望输出：11,11
```
