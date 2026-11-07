# 732 duel_catdiff

Catalan 差式对峙：差式 vs 商式（`n=5：42 对 42`；`n=6：132 对 132`）。

- 对标：731_cat_diff → 差源互补（727 商源合流；跨算法双边锁定）
- 绕行：`252-210=42` 正数差（436 口径）；商式端精确整除（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/732_duel_catdiff/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/732_duel_catdiff/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/732_duel_catdiff/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/732_duel_catdiff/main.sla -o /tmp/basic_732 && /tmp/basic_732
# 期望输出：42,42,132,132
```
