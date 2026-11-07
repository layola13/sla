# 395 tri coprime

互素和三向对峙（循环 vs 公式 vs 逆序；381 的三向版）。

- 对标：381 coprime_sum → `n·φ(n)/2` 闭式锁定（`9 → 27` 三向）
- 绕行：`phi(1)=1` 约定复用（382 口径）；逆序有符号 `int`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/395_tri_coprime/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/395_tri_coprime/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/395_tri_coprime/main.sla -o /tmp/basic_395 && /tmp/basic_395
# 期望输出：27,27,27
```
