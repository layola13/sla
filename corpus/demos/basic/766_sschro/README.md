# 766 sschro

小 Schröder 数短递推 `s(n)=((6n-3)·s(n-1)-(n-2)·s(n-2))/(n+1)`（精确整除）。
`s(2)=3`，`s(3)=11`，`s(4)=45`。

- 对标：606_schroder → 大数姐妹序列（`S(n)=2s(n)`；769 做短递推/半大数对峙）
- 绕行：`s(0)=1`、`s(1)=1` 双基守卫；分子整体先算后除（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/766_sschro/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/766_sschro/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/766_sschro/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/766_sschro/main.sla -o /tmp/basic_766 && /tmp/basic_766
# 期望输出：3,11,45
```
