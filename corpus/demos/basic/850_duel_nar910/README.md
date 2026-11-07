# 850 duel_nar910

Narayana 高位二对峙：行和 vs Catalan 商式（`n=8：1430` 双锁，`n=9：4862` 双锁）。

- 对标：847_nar_row9 × 720_duel_nar8 → 行源与对峙体例合流（855 做三向）
- 绕行：`C(18,9)/10=4862` 商式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/850_duel_nar910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/850_duel_nar910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/850_duel_nar910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/850_duel_nar910/main.sla -o /tmp/basic_850 && /tmp/basic_850
# 期望输出：1430,1430,4862,4862
```
