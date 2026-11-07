# 591 duel_fubini

Fubini 对峙：行和式 `Σk!·S(n,k) ==` 组合递推 `F(n)=Σ_{k<n}C(n,k)·F(k)`（`n=4`：`75` 对 `75`；`n=5`：`541` 对 `541`）。

- 对标：588_fubini → 单实现见源（286 系对峙体例；`F(0)=1` 空排基准）
- 绕行：`f` 表 `i32` 槽存 int 累加（528 同例混算）；`F(5)=1+5+30+130+375` 分项复核

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/591_duel_fubini/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/591_duel_fubini/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/591_duel_fubini/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/591_duel_fubini/main.sla -o /tmp/basic_591 && /tmp/basic_591
# 期望输出：75,75,541,541
```
