# 918 duel_eul9

Eulerian 续高位三对峙：建表 vs 交错和（`(9,2)=14608` 双锁，`(9,3)=88234` 双锁）。

- 对标：916_eul9 × 921_eul_alt9 → 高位源与交错源合流（799 体例续高位）
- 绕行：`%2` 判奇偶交错求和（805 体例复用；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/918_duel_eul9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/918_duel_eul9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/918_duel_eul9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/918_duel_eul9/main.sla -o /tmp/basic_918 && /tmp/basic_918
# 期望输出：14608,14608,88234,88234
```
