# 531 stirling2

第二类 Stirling 数 `S(n,k)=k*S(n-1,k)+S(n-1,k-1)`（`S(5,2)=15`，`S(5,3)=25`，`S(6,3)=90`）。

- 对标：351_catalan → 同为组合三角递推（351 走卷积，此处走加权递推）
- 绕行：二维表拍扁为一维 Vec（下标 `nn*w+kk` 纯整数运算，避嵌套数组 #20）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/531_stirling2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/531_stirling2/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/531_stirling2/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/531_stirling2/main.sla -o /tmp/basic_531 && /tmp/basic_531
# 期望输出：15,25,90
```
