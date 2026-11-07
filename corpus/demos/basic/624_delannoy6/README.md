# 624 delannoy6

Delannoy 高位二 `D(6,6)=8989`（`D(6,5)=3653` 低位锚；`D(6,4)=1289` 中继）。

- 对标：617_delannoy_high → 高位版互补（同式再上一位；625 做三向收束）
- 绕行：`3653+1683+3653=8989` 三项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/624_delannoy6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/624_delannoy6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/624_delannoy6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/624_delannoy6/main.sla -o /tmp/basic_624 && /tmp/basic_624
# 期望输出：3653,8989
```
