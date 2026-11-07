# 475 tri star

星形数三向对峙（闭式 vs 累加 vs 逆序；474 的三向版）。

- 对标：474 duel_star → 逆序三角第三角（`4 → 73` 三向）
- 绕行：逆序有符号 `int`（365 同例）；`n <= 1` 双边守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/475_tri_star/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/475_tri_star/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/475_tri_star/main.sla -o /tmp/basic_475 && /tmp/basic_475
# 期望输出：73,73,73
```
