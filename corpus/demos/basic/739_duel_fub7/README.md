# 739 duel_fub7

有序 Bell 双实现对峙：二项递推 vs 加权和（`n=6：4683 对 4683`；`n=7：47293 对 47293`）。
加权和 `F(n)=sum k!·S(n,k)`（`n=7：1+126+1806+8400+16800+15120+5040=47293`）。

- 对标：736_fub7 → 递推源（加权和版互补；591 体例续高位，双边同值锁定）
- 绕行：加权七项手算复核（阶乘笔误自纠两则，探针三线绿定稿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/739_duel_fub7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/739_duel_fub7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/739_duel_fub7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/739_duel_fub7/main.sla -o /tmp/basic_739 && /tmp/basic_739
# 期望输出：4683,4683,47293,47293
```
