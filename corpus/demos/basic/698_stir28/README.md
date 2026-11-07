# 698 stir28

Stirling 二类第 8 行高位 `S(8,2)=127`，`S(8,3)=966`，`S(8,4)=1701`（同式再上一位）。

- 对标：696_stir27 → `n=7` 版互补（同式再上一位；700 做高位对峙）
- 绕行：`63+3·301=966` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/698_stir28/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/698_stir28/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/698_stir28/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/698_stir28/main.sla -o /tmp/basic_698 && /tmp/basic_698
# 期望输出：127,966,1701
```
