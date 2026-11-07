# 843 duel_lahcol29

Lah 次对角列对峙：建表 vs `n(n-1)`（`(8,7)=56` 双锁，`(9,8)=72` 双锁）。

- 对标：842_lah_col29 → 列源互补（次对角恒等 `L(n,n-1)=n(n-1)` 双值连验）
- 绕行：`9·8=72` 边式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/843_duel_lahcol29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/843_duel_lahcol29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/843_duel_lahcol29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/843_duel_lahcol29/main.sla -o /tmp/basic_843 && /tmp/basic_843
# 期望输出：56,56,72,72
```
