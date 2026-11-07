# 416 sphenic

楔形数（三不同素数之积；386 的等三版）。

- 对标：386 omega → `ω==3` 且无平方因子（`60 → 0`  squared 淘汰）
- 绕行：单语句 `while` 体后不加 `;`（386 同例）；嵌套 `if` 代替 `&&`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/416_sphenic/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/416_sphenic/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/416_sphenic/main.sla -o /tmp/basic_416 && /tmp/basic_416
# 期望输出：1,1,0,1
```
