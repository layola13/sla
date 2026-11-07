# 564 duel_fuss

Fuss 对峙：商式 `C(3n,n)/(2n+1) ==` 三元卷积 `T(n)=Σ_{i+j+k=n-1}T(i)T(j)T(k)`（`n=2`：`3` 对 `3`；`n=3`：`12` 对 `12`）。

- 对标：560_fuss → 单实现见源（286 系对峙体例；三重循环小规模一次绿）
- 绕行：内层上界 `b < m - a` 收窄（和式 `a+b≤m-1` 天然约束，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/564_duel_fuss/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/564_duel_fuss/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/564_duel_fuss/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/564_duel_fuss/main.sla -o /tmp/basic_564 && /tmp/basic_564
# 期望输出：3,3,12,12
```
