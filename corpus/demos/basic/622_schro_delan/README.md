# 622 schro_delan

Schröder–Delannoy 桥式 `S(n)=D(n,n)-D(n+1,n-1)`（`n=2：6`，`n=3：22`，`n=4：90`）。

- 对标：606_schroder × 611_delannoy_tab → 两族桥接（差式单锁；623 做桥对峙）
- 绕行：`D(6,4)=1289` 高位差项（`1683-1289=394`；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/622_schro_delan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/622_schro_delan/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/622_schro_delan/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/622_schro_delan/main.sla -o /tmp/basic_622 && /tmp/basic_622
# 期望输出：6,22,90
```
