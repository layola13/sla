# 639 duel_rio

Riordan 双实现对峙：短递推 vs Motzkin 卷积（`n=4：3 对 3`；`n=5：6 对 6`）。
卷积 `R(n)=sum R(k)·M(n-2-k)`（`n=5：4+0+1+1=6`）。

- 对标：636_riordan → 短递推源（卷积版互补；286 系对峙体例，双边同值锁定）
- 绕行：卷积递归深度止 `n=5`（探针两后端 8s 级，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/639_duel_rio/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/639_duel_rio/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/639_duel_rio/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/639_duel_rio/main.sla -o /tmp/basic_639 && /tmp/basic_639
# 期望输出：3,3,6,6
```
