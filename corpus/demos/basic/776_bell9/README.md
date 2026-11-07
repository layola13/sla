# 776 bell9

Bell 续高位二项递推 `bell(n+1)=sum C(n,k)·bell(k)`（`B(8)=4140`，`B(9)=21147`）。

- 对标：703_stir2_bell → `n≤8` 版互补（同式再上一位；779 做递推/三角对峙）
- 绕行：九项和 `1+8+56+280+1050+2912+5684+7016+4140=21147` 手算复核

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/776_bell9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/776_bell9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/776_bell9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/776_bell9/main.sla -o /tmp/basic_776 && /tmp/basic_776
# 期望输出：4140,21147
```
