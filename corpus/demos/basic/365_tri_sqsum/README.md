# 365 tri sqsum

平方和三向对峙（循环 vs 公式 vs 逆序累加；288 的平方和版）。

- 对标：288 循环公式对峙 → `10 → 385=385=385` 三向锁定
- 绕行：无（`sq_acc` 逆序有符号 `int` 递减，`i >= 1` 到 0 即停，无 160 式无符号回绕坑）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/365_tri_sqsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/365_tri_sqsum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/365_tri_sqsum/main.sla -o /tmp/basic_365 && /tmp/basic_365
# 期望输出：385,385,385
```
