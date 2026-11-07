# 386 omega

不同素因子计数 ω（`ω(30)=3`；376 的去重版）。

- 对标：376 semiprime → 内层去重循环（`12 → 2`，`8 → 1`）
- 绕行：单语句 `while` 体后不加 `;`（`if` 要求 `;`、`while` 禁止，多语句体同例；写法约束）
- 绕行：无（嵌套 while 试除；余因子收尾）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/386_omega/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/386_omega/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/386_omega/main.sla -o /tmp/basic_386 && /tmp/basic_386
# 期望输出：2,3,1
```
