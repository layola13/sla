# 829 duel_stir29

Stirling 二类续高位对峙：建表 vs 递归（`(10,2)=511` 双锁，`(10,3)=9330` 双锁）。

- 对标：826_stir29 → 建表源（递归版互补；830 做内格对峙）
- 绕行：递归三守卫习语（`k=0` 嵌套判 `n` + 越界 + 对角，探针双后端秒级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/829_duel_stir29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/829_duel_stir29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/829_duel_stir29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/829_duel_stir29/main.sla -o /tmp/basic_829 && /tmp/basic_829
# 期望输出：511,511,9330,9330
```
