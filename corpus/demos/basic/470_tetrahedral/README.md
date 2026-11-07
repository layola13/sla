# 470 tetrahedral

四面体数（`T(5)=35`；137 的堆叠版）。

- 对标：137 tri → 三连乘闭式（`T(4)=20`）
- 绕行：无（纯 `int` 闭式；三连乘恒被 6 整除）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/470_tetrahedral/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/470_tetrahedral/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/470_tetrahedral/main.sla -o /tmp/basic_470 && /tmp/basic_470
# 期望输出：35,20,1
```
