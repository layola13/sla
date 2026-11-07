# 543 duel_p

全分拆双实现对峙：背包 DP `==` 五边形递推（`n=10`：`42` 对 `42`；`n=8,9` 同锁）。

- 对标：536_p8 × 542_pent_extend → 两源单实现见各源（286 系对峙体例）
- 绕行：同文件双表各自独立（534 同例）；545 做三向收束

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/543_duel_p/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/543_duel_p/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/543_duel_p/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/543_duel_p/main.sla -o /tmp/basic_543 && /tmp/basic_543
# 期望输出：42,42,22,22
```
