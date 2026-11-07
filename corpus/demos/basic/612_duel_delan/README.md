# 612 duel_delan

Delannoy 对峙：拍扁建表 vs 二项直和 `D(m,n)=sum C(m,k)·C(n,k)·2^k`。
`D(2,2)：13 对 13`；`D(3,3)：63 对 63`。

- 对标：611_delannoy_tab → 建表源（直和式互补；286 系对峙体例，双边同值锁定）
- 绕行：`2^k` 用循环连乘（`>>` 无 token，515 口径）；`k<=n` 内层守卫防越界求和

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/612_duel_delan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/612_duel_delan/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/612_duel_delan/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/612_duel_delan/main.sla -o /tmp/basic_612 && /tmp/basic_612
# 期望输出：13,13,63,63
```
