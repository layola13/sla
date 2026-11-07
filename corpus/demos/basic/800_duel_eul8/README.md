# 800 duel_eul8

Eulerian 续高位二对峙：建表 vs 交错和（`(8,2)：4293 对 4293`；`(8,3)：15619 对 15619`）。

- 对标：798_eul8 → 高位源互补（799 对峙体例上移；双边同值锁定）
- 绕行：`(8,2)` 交错四项 `6561-2304+36=4293` 手算复核（探针同形三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/800_duel_eul8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/800_duel_eul8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/800_duel_eul8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/800_duel_eul8/main.sla -o /tmp/basic_800 && /tmp/basic_800
# 期望输出：4293,4293,15619,15619
```
