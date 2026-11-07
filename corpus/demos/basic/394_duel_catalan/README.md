# 394 duel catalan

Catalan 双实现对峙（商式 vs 递推；351 的对峙版）。

- 对标：351 catalan → 递推 `C_{n+1} = C_n·2(2n+1)/(n+2)` 一致性锁定（`5 → 42=42`）
- 绕行：无（每步整除精确；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/394_duel_catalan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/394_duel_catalan/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/394_duel_catalan/main.sla -o /tmp/basic_394 && /tmp/basic_394
# 期望输出：42,42,14
```
