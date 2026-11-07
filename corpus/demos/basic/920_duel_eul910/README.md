# 920 duel_eul910

Eulerian 续高位四对峙：建表 vs 交错和（`(9,4)=156190` 双锁，`(9,5)=88234` 双锁）。

- 对标：917_eul_row9 × 918_duel_eul9 → 内格源与对峙体例合流（800 体例续高位）
- 绕行：交错大数中间值值域内（`5^9·C(10,k)` 未溢出；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/920_duel_eul910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/920_duel_eul910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/920_duel_eul910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/920_duel_eul910/main.sla -o /tmp/basic_920 && /tmp/basic_920
# 期望输出：156190,156190,88234,88234
```
