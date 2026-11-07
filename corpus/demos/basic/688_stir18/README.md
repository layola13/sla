# 688 stir18

Stirling 一类第 8 行高位 `c(8,1)=5040`，`c(8,2)=13068`，`c(8,3)=13132`（同式再上一位）。

- 对标：686_stir17 → `n=7` 版互补（同式再上一位；690 做高位对峙）
- 绕行：`1764+7·1624=13132` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/688_stir18/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/688_stir18/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/688_stir18/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/688_stir18/main.sla -o /tmp/basic_688 && /tmp/basic_688
# 期望输出：5040,13068,13132
```
