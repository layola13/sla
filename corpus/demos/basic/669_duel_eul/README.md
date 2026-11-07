# 669 duel_eul

Eulerian 双实现对峙：建表 vs 交错和（`(4,2)：11 对 11`；`(5,2)：66 对 66`）。
交错和 `E(n,m)=sum (-1)^k·C(n+1,k)·(m+1-k)^n`（`(5,2)：243-192+15=66`）。

- 对标：666_eulerian → 建表源（交错和版互补；286 系对峙体例，双边同值锁定）
- 绕行：`%2` 判奇偶（515 口径）；`0^5=0` 尾项自然归零

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/669_duel_eul/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/669_duel_eul/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/669_duel_eul/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/669_duel_eul/main.sla -o /tmp/basic_669 && /tmp/basic_669
# 期望输出：11,11,66,66
```
