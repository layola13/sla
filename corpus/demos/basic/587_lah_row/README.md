# 587 lah_row

Lah 行：`n=4` 行 `24,36,12,1`（和 `73`）；`L(5,5)=1` 对角端点。

- 对标：586_lah → 单格版互补（532 行和体例；行和 main 直打）
- 绕行：行和表达式内联（`24+36+12+1=73` 手算复核，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/587_lah_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/587_lah_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/587_lah_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/587_lah_row/main.sla -o /tmp/basic_587 && /tmp/basic_587
# 期望输出：24,36,12,1,73
```
