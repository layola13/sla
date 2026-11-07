# 892 delan_central8

Delannoy 续中央列 `8989,48639,265729`（`D(6,6)`，`D(7,7)`，`D(8,8)`；中央序列复算）。

- 对标：762_delan_central7 → 低位中央版互补（中央序列上移一位；895 做三向收束）
- 绕行：`265729-48639=217090` 中央差手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/892_delan_central8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/892_delan_central8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/892_delan_central8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/892_delan_central8/main.sla -o /tmp/basic_892 && /tmp/basic_892
# 期望输出：8989,48639,265729
```
