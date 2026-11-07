# 405 duel euler

欧拉准则对峙（幂模 vs Legendre；403 的二次剩余版）。

- 对标：403 duel_fermat → `2^((p-1)/2) mod p` 与 `(2|p)` 锁定（`7 → 1=1`，`11 → 0=0`）
- 绕行：无（`p % 8` 查表；纯 `int` 运算）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/405_duel_euler/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/405_duel_euler/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/405_duel_euler/main.sla -o /tmp/basic_405 && /tmp/basic_405
# 期望输出：1,1,0,0
```
