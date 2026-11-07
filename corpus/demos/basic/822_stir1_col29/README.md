# 822 stir1_col29

Stirling 一类第 2 列纵深 `c(7,2)=1764`，`c(8,2)=13068`，`c(9,2)=109584`（列纵深续高位）。

- 对标：816_stir19 → 单格版互补（同式列复算；纵深三值锁定）
- 绕行：`c(6,2)=274` 小值守卫；列值单调递增天然成立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/822_stir1_col29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/822_stir1_col29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/822_stir1_col29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/822_stir1_col29/main.sla -o /tmp/basic_822 && /tmp/basic_822
# 期望输出：1764,13068,109584
```
