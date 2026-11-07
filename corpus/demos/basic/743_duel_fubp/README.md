# 743 duel_fubp

有序 Bell 表商对峙：表版 vs 商版（`n=6：4683 对 4683`；`n=7：47293 对 47293`）。

- 对标：742_fubp → 表源互补（736 商源合流；同核异源双边锁定）
- 绕行：同核（`F(n)=sum C·F`）异源（表/商）交叉确证系数源（730 体例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/743_duel_fubp/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/743_duel_fubp/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/743_duel_fubp/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/743_duel_fubp/main.sla -o /tmp/basic_743 && /tmp/basic_743
# 期望输出：4683,4683,47293,47293
```
