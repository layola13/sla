# 862 duel_catdiff9

Catalan 差式对峙：差式 vs 商式（`n=9：4862` 双锁，`n=10：16796` 双锁）。

- 对标：861_cat_diff9 × 856_cat9 → 差源与商源合流（732 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/862_duel_catdiff9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/862_duel_catdiff9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/862_duel_catdiff9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/862_duel_catdiff9/main.sla -o /tmp/basic_862 && /tmp/basic_862
# 期望输出：4862,4862,16796,16796
```
