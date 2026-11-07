# 723 nar_col2

Narayana 第 2 列 `50,105,196`（`N(6,2)=50`，`N(7,2)=105`，`N(8,2)=196`；列纵深）。

- 对标：716_nar7 → 单格版互补（同式列复算；纵深三值锁定）
- 绕行：`N(5,2)=20` 小值守卫（598 低位锚复用）；列值单调递增天然成立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/723_nar_col2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/723_nar_col2/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/723_nar_col2/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/723_nar_col2/main.sla -o /tmp/basic_723 && /tmp/basic_723
# 期望输出：50,105,196
```
