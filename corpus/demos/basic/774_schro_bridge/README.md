# 774 schro_bridge

大小 Schröder 桥恒等 `S(n)=2·s(n)`：`S(6)=1806`，`S(7)=8558`，`S(8)=41586`（桥大端三锁）。

- 对标：769_duel_sschro → 对峙版互补（桥恒等大端复算；775 做三向收束）
- 绕行：乘式断言 `sch(n) != 2 * ssho(n)`（条件内乘法，`2*s` 恒偶天然成立）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/774_schro_bridge/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/774_schro_bridge/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/774_schro_bridge/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/774_schro_bridge/main.sla -o /tmp/basic_774 && /tmp/basic_774
# 期望输出：1806,8558,41586
```
