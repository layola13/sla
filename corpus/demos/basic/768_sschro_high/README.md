# 768 sschro_high

小 Schröder 高位 `s(7)=4279`（`s(6)=903` 低位锚；短递推一步 `[39·903-985]/8`）。

- 对标：766_sschro → 低位版互补（同式高位复算；770 做高位对峙）
- 绕行：`[35217-985]/8=4279` 分子先算后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/768_sschro_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/768_sschro_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/768_sschro_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/768_sschro_high/main.sla -o /tmp/basic_768 && /tmp/basic_768
# 期望输出：903,4279
```
