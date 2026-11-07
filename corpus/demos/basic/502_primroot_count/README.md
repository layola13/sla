# 502 primroot count

本原根计数（`p=11 → 4`；485 的计数版）。

- 对标：485 primroot → 阶扫描计数（`=φ(φ(p))` 实例）
- 绕行：无（复用 `order_mod`；小素数三重循环可承受）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/502_primroot_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/502_primroot_count/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/502_primroot_count/main.sla -o /tmp/basic_502 && /tmp/basic_502
# 期望输出：2,4,2
```
