# 638 rio_high

Riordan 高位 `R(6)=15`（`R(5)=6` 低位锚；短递推一步 `5·21/7`）。

- 对标：636_riordan → 低位版互补（同式高位复算；640 做高位对峙）
- 绕行：`5·(2·6+3·3)/7=5·21/7=15` 先乘后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/638_rio_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/638_rio_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/638_rio_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/638_rio_high/main.sla -o /tmp/basic_638 && /tmp/basic_638
# 期望输出：6,15
```
