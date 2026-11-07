# 889 duel_delan8

Delannoy 续高位对峙三：建表 vs 直和（`(8,7)=108545` 双锁，`(8,8)=265729` 双锁）。

- 对标：888_delan_high8 × 759_duel_delan7 → 高位源与对峙体例合流
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/889_duel_delan8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/889_duel_delan8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/889_duel_delan8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/889_duel_delan8/main.sla -o /tmp/basic_889 && /tmp/basic_889
# 期望输出：108545,108545,265729,265729
```
