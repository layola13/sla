# 890 duel_delan86

Delannoy 续高位对峙四：建表 vs 直和（`(8,5)=13073` 双锁，`(8,6)=40081` 双锁）。

- 对标：887_delan_row8 × 889_duel_delan8 → 内格源与对峙体例合流（895 做三向）
- 绕行：`1+80+1120+4480+5600+1792=13073` 直和逐项复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/890_duel_delan86/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/890_duel_delan86/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/890_duel_delan86/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/890_duel_delan86/main.sla -o /tmp/basic_890 && /tmp/basic_890
# 期望输出：13073,13073,40081,40081
```
