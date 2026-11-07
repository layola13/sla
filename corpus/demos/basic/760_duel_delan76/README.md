# 760 duel_delan76

Delannoy 续高位对峙二：建表 vs 直和（`(7,5)：7183 对 7183`；`(7,6)：19825 对 19825`）。

- 对标：758_delan_high7 → 高位源互补（759 对峙体例内移；双边同值锁定）
- 绕行：直和七项 `1+84+1260+5600+8400+4032+448=19825` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/760_duel_delan76/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/760_duel_delan76/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/760_duel_delan76/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/760_duel_delan76/main.sla -o /tmp/basic_760 && /tmp/basic_760
# 期望输出：7183,7183,19825,19825
```
