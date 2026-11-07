# 790 duel_schro10

大 Schröder 续高位二对峙：迭代 vs 商式（`n=9：206098 对 206098`；`n=10：1037718 对 1037718`）。

- 对标：788_schro10 → 高位源互补（789 对峙体例上移；双边同值锁定）
- 绕行：商式十一项 `1+55+990+8580+42042+126126+240240+291720+218790+92378+16796` 手算复核

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/790_duel_schro10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/790_duel_schro10/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/790_duel_schro10/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/790_duel_schro10/main.sla -o /tmp/basic_790 && /tmp/basic_790
# 期望输出：206098,206098,1037718,1037718
```
