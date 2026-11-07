# 546 pascal

Pascal 递推建表 `C(n,k)=C(n-1,k-1)+C(n-1,k)`（拍扁二维表，531 范式）。
`C(8,4)=70`，`C(9,4)=126`，`C(10,5)=252`。

- 对标：531_stirling2 → 同为拍扁加权三角（531 首列恒零，此处首列恒一）
- 绕行：初版沿 531 范式首列留零致 panic 54601（`C(nn,0)` 恒 1，补 `t[nn*w]=1` 修正，后续 Pascal 系各源同式）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/546_pascal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/546_pascal/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/546_pascal/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/546_pascal/main.sla -o /tmp/basic_546 && /tmp/basic_546
# 期望输出：70,126,252
```
