# 894 duel_delan_col8

Delannoy 列对峙二：建表 vs 直和（`(6,8)=40081` 双锁，`(7,8)=108545` 双锁）。

- 对标：893_delan_col8 × 889_duel_delan8 → 列源与对峙体例合流（764 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/894_duel_delan_col8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/894_duel_delan_col8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/894_duel_delan_col8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/894_duel_delan_col8/main.sla -o /tmp/basic_894 && /tmp/basic_894
# 期望输出：40081,40081,108545,108545
```
