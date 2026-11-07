# 824 stir19_right

Stirling 一类第 9 行右部 `c(9,5)=22449`，`c(9,6)=4536`，`c(9,7)=546`（右半行复算）。

- 对标：816_stir19 → 左部版互补（右半行复算；825 做三向收束）
- 绕行：`c(9,8)=36` 右端守卫；右部三值与左部异构互补（全行覆盖 816+824）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/824_stir19_right/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/824_stir19_right/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/824_stir19_right/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/824_stir19_right/main.sla -o /tmp/basic_824 && /tmp/basic_824
# 期望输出：22449,4536,546
```
