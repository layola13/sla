# 833 duel_col210

Stirling 二类次对角列对峙：建表 vs 三角数 `n(n-1)/2`（`(9,8)=36` 双锁，`(10,9)=45` 双锁）。

- 对标：832_stir2_col210 → 列源互补（次对角恒等 `S(n,n-1)=C(n,2)` 双值连验）
- 绕行：`10·9/2=45` 三角数手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/833_duel_col210/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/833_duel_col210/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/833_duel_col210/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/833_duel_col210/main.sla -o /tmp/basic_833 && /tmp/basic_833
# 期望输出：36,36,45,45
```
