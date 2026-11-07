# 664 fine8

Fine 高位三 `F(8)=622`（`F(7)=186` 低位锚；短递推一步 `[30·429-1674]/18`）。

- 对标：661_fine7 → 高位版互补（同式再上一位；665 做三向收束）
- 绕行：`[12870-1674]/18=622` 分子先算后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/664_fine8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/664_fine8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/664_fine8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/664_fine8/main.sla -o /tmp/basic_664 && /tmp/basic_664
# 期望输出：186,622
```
