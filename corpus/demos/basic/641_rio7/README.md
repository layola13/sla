# 641 rio7

Riordan 高位二 `R(7)=36`（`R(6)=15` 低位锚；短递推一步 `6·48/8`）。

- 对标：638_rio_high → 高位版互补（同式再上一位；642 做高位二对峙）
- 绕行：`6·(2·15+3·6)/8=6·48/8=36` 先乘后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/641_rio7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/641_rio7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/641_rio7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/641_rio7/main.sla -o /tmp/basic_641 && /tmp/basic_641
# 期望输出：15,36
```
