# 534 euler_duel

Euler 定理对峙：互异分拆 `q(n) ==` 奇部数分拆 `p_odd(n)`（`n=6`：`4` 对 `4`；`n=7`：`5` 对 `5`）。

- 对标：526_distinct × 527_oddpart → 双实现一致性锁定（单实现见各源；286 系对峙体例）
- 绕行：同文件双 DP 各自建表（避数组形参调用者可见可变语义纠缠，127 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/534_euler_duel/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/534_euler_duel/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/534_euler_duel/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/534_euler_duel/main.sla -o /tmp/basic_534 && /tmp/basic_534
# 期望输出：4,4,5,5
```
