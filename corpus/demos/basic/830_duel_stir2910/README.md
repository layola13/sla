# 830 duel_stir2910

Stirling 二类续高位二对峙：建表 vs 递归（`(10,4)=34105` 双锁，`(9,4)=7770` 双锁）。

- 对标：826_stir29 × 829_duel_stir29 → 内格源与对峙体例合流（835 做三向）
- 绕行：`966+4·1701=7770` 双项手算复核（探针双后端秒级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/830_duel_stir2910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/830_duel_stir2910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/830_duel_stir2910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/830_duel_stir2910/main.sla -o /tmp/basic_830 && /tmp/basic_830
# 期望输出：34105,34105,7770,7770
```
