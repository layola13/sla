# 846 nar9

Narayana 第 9 行左部 `N(9,1)=36`，`N(9,2)=336`，`N(9,3)=1176`（商式，同式再上一位）。

- 对标：718_nar8 → `n=8` 版互补（同式再上一位；849 做商表对峙）
- 绕行：`36·84/9=336` 先乘后除精确整除（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/846_nar9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/846_nar9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/846_nar9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/846_nar9/main.sla -o /tmp/basic_846 && /tmp/basic_846
# 期望输出：36,336,1176
```
