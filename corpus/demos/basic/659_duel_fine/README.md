# 659 duel_fine

Fine 双实现对峙：短递推 vs 桥序（`n=4：6 对 6`；`n=5：18 对 18`）。
桥序 `F(n)=(Catalan(n)-F(n-1))/2`（`C(5)-F(4)=42-6=36`，半之 `18`）。

- 对标：656_fine → 短递推源（桥序版互补；286 系对峙体例，双边同值锁定）
- 绕行：桥恒等 `C(n)=2F(n)+F(n-1)` 六值连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/659_duel_fine/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/659_duel_fine/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/659_duel_fine/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/659_duel_fine/main.sla -o /tmp/basic_659 && /tmp/basic_659
# 期望输出：6,6,18,18
```
