# 658 fine_high

Fine 高位 `F(6)=57`（`F(5)=18` 低位锚；短递推一步 `[22·42-126]/14`）。

- 对标：656_fine → 低位版互补（同式高位复算；660 做高位对峙）
- 绕行：`[924-126]/14=57` 分子先算后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/658_fine_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/658_fine_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/658_fine_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/658_fine_high/main.sla -o /tmp/basic_658 && /tmp/basic_658
# 期望输出：18,57
```
