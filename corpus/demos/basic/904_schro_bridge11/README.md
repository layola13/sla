# 904 schro_bridge11

大小 Schröder 续桥恒等 `S(n)=2s(n)`：`S(11)=5293446`，`S(12)=27297738`（桥续大端双锁）。

- 对标：794_schro_bridge9 → `n=9/10` 桥互补（桥恒等续高位复算）
- 绕行：无（桥恒等双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/904_schro_bridge11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/904_schro_bridge11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/904_schro_bridge11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/904_schro_bridge11/main.sla -o /tmp/basic_904 && /tmp/basic_904
# 期望输出：5293446,5293446,27297738,27297738
```
