# 575 tri_derange

`!6=265` 三向收束：双项递推 `==` 容斥和式 `==` 单项递推 `!n=n·!(n-1)+(-1)^n`（`265` 对 `265` 对 `265`）。

- 对标：566_derange × 567_derange_sum → 两源见各源（单项式为第三源；316 系三向体例）
- 绕行：单项式另断言 `derange1(5)=44`（`5·9-1` 手算复核）；`n=0` 空循环返 1 天然

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/575_tri_derange/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/575_tri_derange/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/575_tri_derange/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/575_tri_derange/main.sla -o /tmp/basic_575 && /tmp/basic_575
# 期望输出：265,265,265
```
