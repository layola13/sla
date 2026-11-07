# 377 sigma

除数和函数 σ（`σ(12)=28`；337 的求和版）。

- 对标：337 div_count → 整除累加（`6 → 12`，`7 → 8`）
- 绕行：无（纯 `int` 循环；`%` 整除判定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/377_sigma/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/377_sigma/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/377_sigma/main.sla -o /tmp/basic_377 && /tmp/basic_377
# 期望输出：28,12,8
```
