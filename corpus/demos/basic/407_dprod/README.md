# 407 dprod

数位积（`12345 → 120`；406 的乘积版）。

- 对标：406 dsum → 乘法累积（`101 → 0` 含零即零）
- 绕行：`n == 0` 提前返回 0（空循环得乘法单位元 1 是错值）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/407_dprod/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/407_dprod/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/407_dprod/main.sla -o /tmp/basic_407 && /tmp/basic_407
# 期望输出：120,0,7
```
