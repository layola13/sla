# 606 schroder

大 Schröder 数递推 `S(n)=S(n-1)+sum S(k)·S(n-1-k)`（迭代建表，546 范式）。
`S(2)=6`，`S(3)=22`，`S(4)=90`。

- 对标：351_catalan → 同为格路计数族（含对角步格路；609 做迭代/递归对峙）
- 绕行：`S(0)=1`、`S(1)=2` 双基守卫（`n<2` 跳建表循环）；`>>`/`^` 未知算子一律绕行（515 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/606_schroder/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/606_schroder/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/606_schroder/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/606_schroder/main.sla -o /tmp/basic_606 && /tmp/basic_606
# 期望输出：6,22,90
```
