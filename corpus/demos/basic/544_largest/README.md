# 544 largest

最大部恰为 `k` 的分拆数 `P(n,k)=P(n-1,k-1)+P(n-k,k)`（`P(10,3)=8`，`P(8,2)=4`，`P(7,3)=4`）。

- 对标：531_stirling2 → 同为拍扁加权递推（531 权为 `k`，此处为 0/1 选择）
- 绕行：上界 `kk <= nn` 收窄（532 同例）；`P(5,5)=1` 端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/544_largest/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/544_largest/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/544_largest/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/544_largest/main.sla -o /tmp/basic_544 && /tmp/basic_544
# 期望输出：8,4,4
```
