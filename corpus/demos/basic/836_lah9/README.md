# 836 lah9

Lah 第 9 行左部 `L(9,2)=1451520`，`L(9,3)=1693440`，`L(9,4)=846720`（建表递推，同式再上一位）。

- 对标：708_lah8 → `n=8` 版互补（同式再上一位；839 做建表/商式对峙）
- 绕行：`C(8,2)·9!/6=1693440` 商式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/836_lah9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/836_lah9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/836_lah9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/836_lah9/main.sla -o /tmp/basic_836 && /tmp/basic_836
# 期望输出：1451520,1693440,846720
```
