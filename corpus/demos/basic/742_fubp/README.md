# 742 fubp

有序 Bell 表版二项递推（Pascal 表供系数；`F(6)=4683`，`F(7)=47293`）。

- 对标：736_fub7 → 商版互补（同核异源，730 体例；743 做表商对峙）
- 绕行：表版与商版系数源异构（建表拍扁 vs 乘式约分，730 同式复用）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/742_fubp/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/742_fubp/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/742_fubp/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/742_fubp/main.sla -o /tmp/basic_742 && /tmp/basic_742
# 期望输出：4683,47293
```
