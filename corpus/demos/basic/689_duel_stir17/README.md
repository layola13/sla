# 689 duel_stir17

Stirling 一类双实现对峙：建表 vs 递归（`(7,2)：1764 对 1764`；`(7,3)：1624 对 1624`）。

- 对标：686_stir17 → 建表源（递归版互补；286 系对峙体例，双边同值锁定）
- 绕行：递归三守卫（`k=0` 嵌套判 `n` + 越界 + 对角；探针两后端 8s 级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/689_duel_stir17/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/689_duel_stir17/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/689_duel_stir17/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/689_duel_stir17/main.sla -o /tmp/basic_689 && /tmp/basic_689
# 期望输出：1764,1764,1624,1624
```
