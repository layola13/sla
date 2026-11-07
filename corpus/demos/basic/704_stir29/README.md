# 704 stir29

Stirling 二类第 9 行高位 `S(9,2)=255`，`S(9,3)=3025`，`S(9,4)=7770`（同式再上一位）。

- 对标：698_stir28 → `n=8` 版互补（同式再上一位；705 做三向收束）
- 绕行：`127+3·966=3025` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/704_stir29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/704_stir29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/704_stir29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/704_stir29/main.sla -o /tmp/basic_704 && /tmp/basic_704
# 期望输出：255,3025,7770
```
