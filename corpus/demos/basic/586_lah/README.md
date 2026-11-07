# 586 lah

Lah 数商式 `L(n,k)=C(n-1,k-1)·n!/k!`（先乘后除精确整除）。
`L(4,2)=36`，`L(4,3)=12`，`L(5,2)=240`。

- 对标：558_ballot → 同为商式族（此处为 Lah 三元商式；590 做递推对峙）
- 绕行：`L(3,2)=6` 小值端点守卫；中间积值域内无溢出（`C(4,1)·120=480`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/586_lah/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/586_lah/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/586_lah/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/586_lah/main.sla -o /tmp/basic_586 && /tmp/basic_586
# 期望输出：36,12,240
```
