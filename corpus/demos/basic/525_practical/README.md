# 525 practical

实际数（Stewart 准则；378 的划分版）。

- 对标：378 abundant → 素数幂 sigma 链式判定（`14 → 0` 在 `7>4` 淘汰）
- 绕行：嵌套 `while` 内层不加 `;`（386 同例）；`n==1` 特判为实用数

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/525_practical/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/525_practical/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/525_practical/main.sla -o /tmp/basic_525 && /tmp/basic_525
# 期望输出：1,0,1,1
```
