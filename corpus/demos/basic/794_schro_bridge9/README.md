# 794 schro_bridge9

大小 Schröder 续桥恒等 `S(n)=2·s(n)`：`S(9)=206098`，`S(10)=1037718`（桥续大端双锁）。

- 对标：774_schro_bridge → 低位桥互补（桥恒等续高位复算；795 做三向收束）
- 绕行：乘式断言 `sch(n) != 2 * ssho(n)`（条件内乘法；大数恒偶天然成立）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/794_schro_bridge9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/794_schro_bridge9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/794_schro_bridge9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/794_schro_bridge9/main.sla -o /tmp/basic_794 && /tmp/basic_794
# 期望输出：206098,1037718
```
