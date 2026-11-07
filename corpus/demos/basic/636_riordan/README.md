# 636 riordan

Riordan 数短递推 `R(n)=(n-1)·(2R(n-1)+3R(n-2))/(n+1)`（先乘后除精确整除）。
`R(3)=1`，`R(4)=3`，`R(5)=6`。

- 对标：626_motzkin → Motzkin 姐妹序列（`M(n)=R(n)+R(n+1)`；639 做短递推/卷积对峙）
- 绕行：`R(0)=1`、`R(1)=0` 双基守卫；除法逐项精确（`9/6` 前先乘得 `36/6`，596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/636_riordan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/636_riordan/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/636_riordan/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/636_riordan/main.sla -o /tmp/basic_636 && /tmp/basic_636
# 期望输出：1,3,6
```
