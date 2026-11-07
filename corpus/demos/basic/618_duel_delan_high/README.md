# 618 duel_delan_high

Delannoy 高位对峙：拍扁建表 vs 二项直和（`(4,4)：321 对 321`；`(5,5)：1683 对 1683`）。

- 对标：617_delannoy_high → 高位源互补（612 对峙体例上移；双边同值锁定）
- 绕行：直和六项 `1+50+400+800+400+32=1683` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/618_duel_delan_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/618_duel_delan_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/618_duel_delan_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/618_duel_delan_high/main.sla -o /tmp/basic_618 && /tmp/basic_618
# 期望输出：321,321,1683,1683
```
