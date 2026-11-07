# 457 sylvester

Sylvester 数列（`s(4)=1807`；446 的乘积递推版）。

- 对标：446 jacobsthal → 平方递推（`s(3)=43`，埃及分数背景）
- 绕行：`n=0` 提前返回 2（空循环初值坑，407 类）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/457_sylvester/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/457_sylvester/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/457_sylvester/main.sla -o /tmp/basic_457 && /tmp/basic_457
# 期望输出：1807,43,2
```
