# 451 bell

Bell 数（`B(5)=52`；351 的集合划分版）。

- 对标：351 catalan → Stirling 递推求和（`S(5,3)=25` 另断言）
- 绕行：递归五基（`k==n/k==1` 归 1，`k==0/k>n` 归 0）；小规模安全

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/451_bell/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/451_bell/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/451_bell/main.sla -o /tmp/basic_451 && /tmp/basic_451
# 期望输出：52,15,25
```
