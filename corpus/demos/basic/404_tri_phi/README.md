# 404 tri phi

phi 三向对峙（循环 vs 积式 vs 逆序；395 的 phi 版）。

- 对标：395 tri_coprime → 积式 `n·Π(1-1/p)` 锁定（`φ(12)=4` 三向）
- 绕行：单语句 `while` 体后不加 `;`（386 同例）；`phi(1)=1` 约定复用

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/404_tri_phi/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/404_tri_phi/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/404_tri_phi/main.sla -o /tmp/basic_404 && /tmp/basic_404
# 期望输出：4,4,4
```
