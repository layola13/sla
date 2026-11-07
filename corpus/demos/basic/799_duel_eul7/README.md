# 799 duel_eul7

Eulerian 续高位对峙：建表 vs 交错和（`(7,2)：1191 对 1191`；`(7,3)：2416 对 2416`）。

- 对标：796_eul7 → 高位源互补（672 交错源合流；669 体例续高位，双边同值锁定）
- 绕行：`(7,2)` 交错四项 `2187-1024+28=1191` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/799_duel_eul7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/799_duel_eul7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/799_duel_eul7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/799_duel_eul7/main.sla -o /tmp/basic_799 && /tmp/basic_799
# 期望输出：1191,1191,2416,2416
```
