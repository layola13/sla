# 835 tri_stir29

Stirling 二类续三向：建表行和 × Bell 二项递推 × Aitken 三角（`n=10：115975` 三锁）。

- 对标：827_stir2_row10 → 行源互补（三实现皆迭代，避递归扇出；776 Bell 体例收束）
- 绕行：Pascal 首列逐行置 1（Stirling 表首列恒零，组合表须首列置 1，探针纠偏；三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/835_tri_stir29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/835_tri_stir29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/835_tri_stir29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/835_tri_stir29/main.sla -o /tmp/basic_835 && /tmp/basic_835
# 期望输出：115975,115975,115975
```
