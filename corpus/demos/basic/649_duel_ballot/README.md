# 649 duel_ballot

Ballot 双实现对峙：拍扁建表 vs 商式（`(4,2)：9 对 9`；`(4,3)：14 对 14`）。
商式 `B(n,k)=(n-k+1)·C(n+k,k)/(n+1)`（先乘后除精确整除）。

- 对标：646_ballot → 建表源（商式版互补；286 系对峙体例，双边同值锁定）
- 绕行：`3·15/5=9`、`2·35/5=14` 先乘后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/649_duel_ballot/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/649_duel_ballot/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/649_duel_ballot/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/649_duel_ballot/main.sla -o /tmp/basic_649 && /tmp/basic_649
# 期望输出：9,9,14,14
```
