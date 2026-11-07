# 541 euler2

Euler 定理高位对峙：`q(n) == p_odd(n)`（`n=8`：`6` 对 `6`；`n=9`：`8` 对 `8`；`n=10`：`10` 对 `10`）。

- 对标：534_euler_duel → 低位版互补（539/540 双源单实现见各源；539 修正值锁定）
- 绕行：同文件双 DP 各自建表（534 同例，避形参可变语义纠缠）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/541_euler2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/541_euler2/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/541_euler2/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/541_euler2/main.sla -o /tmp/basic_541 && /tmp/basic_541
# 期望输出：6,6,8,8,10,10
```
