# 476 egypt

贪心埃及分数（`4/7 → 首 2，共 2 项`；362 的单位分数版）。

- 对标：362 harm_bound → 贪心展开计数（`3/7 → 3 项 [3,11,231]`）
- 绕行：步步约分（gcd 内联；`n==0` 终局判停）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/476_egypt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/476_egypt/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/476_egypt/main.sla -o /tmp/basic_476 && /tmp/basic_476
# 期望输出：2,2,3
```
