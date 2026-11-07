# 736 fub7

有序 Bell 二项递推 `F(n)=sum C(n,k)·F(k)`（`F(6)=4683`，`F(7)=47293`；593 体例续高位）。

- 对标：588_fubini → 低位加权版互补（此处为高位递推；739 做递推/加权对峙）
- 绕行：`F(0)=1` 空和守卫前置；`F(5)=541` 低位锚守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/736_fub7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/736_fub7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/736_fub7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/736_fub7/main.sla -o /tmp/basic_736 && /tmp/basic_736
# 期望输出：4683,47293
```
