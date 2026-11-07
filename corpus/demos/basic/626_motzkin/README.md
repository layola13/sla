# 626 motzkin

Motzkin 数递推 `M(n)=M(n-1)+sum M(k)·M(n-2-k)`（迭代建表，546 范式）。
`M(3)=4`，`M(4)=9`，`M(5)=21`。

- 对标：606_schroder → 同为格路计数族（含平步格路；629 做迭代/递归对峙）
- 绕行：`M(0)=1`、`M(1)=1` 双基守卫；内层上界 `ii-2`（`n=2` 只计 `k=0` 一项）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/626_motzkin/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/626_motzkin/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/626_motzkin/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/626_motzkin/main.sla -o /tmp/basic_626 && /tmp/basic_626
# 期望输出：4,9,21
```
