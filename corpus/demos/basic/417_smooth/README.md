# 417 smooth

y-光滑数（`36 → 1`，`42 → 0`；416 的界版）。

- 对标：416 sphenic → 试除中途判界（`49` 对 `7/5` 分界验证）
- 绕行：单语句 `while` 体后不加 `;`；余因子另判界

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/417_smooth/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/417_smooth/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/417_smooth/main.sla -o /tmp/basic_417 && /tmp/basic_417
# 期望输出：1,0,1
```
