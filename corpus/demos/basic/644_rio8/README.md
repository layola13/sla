# 644 rio8

Riordan 高位三 `R(8)=91`（`R(7)=36` 低位锚；短递推一步 `7·117/9`）。

- 对标：641_rio7 → 高位版互补（同式再上一位；645 做三向收束）
- 绕行：`7·(2·36+3·15)/9=7·117/9=91` 先乘后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/644_rio8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/644_rio8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/644_rio8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/644_rio8/main.sla -o /tmp/basic_644 && /tmp/basic_644
# 期望输出：36,91
```
