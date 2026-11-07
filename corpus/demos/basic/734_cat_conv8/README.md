# 734 cat_conv8

Catalan 卷积高位 `C(7)=429`，`C(8)=1430`（同式再上两位；556 体例高位）。

- 对标：726_cat_conv6 → 低位卷积版互补（同式再上两位；735 做三向收束）
- 绕行：`catc(8)` 卷积扇出探针两后端 8s 级（含全组断言，无超时风险）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/734_cat_conv8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/734_cat_conv8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/734_cat_conv8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/734_cat_conv8/main.sla -o /tmp/basic_734 && /tmp/basic_734
# 期望输出：429,1430
```
