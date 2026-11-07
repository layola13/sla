# 508 bitcount

位计数 popcount（`255 → 8`；406 的二进制版）。

- 对标：406 dsum → `n&(n-1)` 消位（`&` 加括号，145 口径；`0 → 0`）
- 绕行：`m != 0` 守卫（零输入空循环得 0 正确，无需特判）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/508_bitcount/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/508_bitcount/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/508_bitcount/main.sla -o /tmp/basic_508 && /tmp/basic_508
# 期望输出：8,3,0
```
