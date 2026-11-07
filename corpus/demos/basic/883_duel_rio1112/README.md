# 883 duel_rio1112

Riordan 续桥二对峙：短递推 vs 桥序（`n=11：1585` 双锁，`n=12：4213` 双锁）。

- 对标：881_motz_riob11 × 753_duel_rio910 → 续桥源与对峙体例合流
- 绕行：桥序 `R(n)=M(n-1)-R(n-1)` 六值连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/883_duel_rio1112/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/883_duel_rio1112/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/883_duel_rio1112/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/883_duel_rio1112/main.sla -o /tmp/basic_883 && /tmp/basic_883
# 期望输出：1585,1585,4213,4213
```
