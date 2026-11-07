# 699 duel_stir27

Stirling 二类双实现对峙：建表 vs 递归（`(7,2)：63 对 63`；`(7,3)：301 对 301`）。

- 对标：696_stir27 → 建表源（递归版互补；286 系对峙体例，双边同值锁定）
- 绕行：递归三守卫（`k=0` 嵌套判 `n` + 越界 + 对角；689 同形复用）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/699_duel_stir27/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/699_duel_stir27/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/699_duel_stir27/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/699_duel_stir27/main.sla -o /tmp/basic_699 && /tmp/basic_699
# 期望输出：63,63,301,301
```
