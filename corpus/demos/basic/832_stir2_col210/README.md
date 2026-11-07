# 832 stir2_col210

Stirling 二类列纵深 `S(7,2)=63`，`S(8,2)=127`，`S(9,2)=255`（`2^(n-1)-1` 上行链）。

- 对标：826_stir29 → 单格版互补（纵深三值锁定；833 做列对峙）
- 绕行：`2^6-1=63` 幂式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/832_stir2_col210/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/832_stir2_col210/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/832_stir2_col210/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/832_stir2_col210/main.sla -o /tmp/basic_832 && /tmp/basic_832
# 期望输出：63,127,255
```
