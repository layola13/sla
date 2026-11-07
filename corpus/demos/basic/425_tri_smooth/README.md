# 425 tri smooth

光滑数三向对峙（试除 vs 最大素因子 vs 除尽；404 的形态版）。

- 对标：404 tri_phi → 三路径锁定（`36 → 1=1=1`，`42 → 0`）
- 绕行：`sm_div` 以 `y` 为除子上界（余 `1` 即光滑）；嵌套 `while` 内层不加 `;`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/425_tri_smooth/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/425_tri_smooth/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/425_tri_smooth/main.sla -o /tmp/basic_425 && /tmp/basic_425
# 期望输出：1,1,1,0
```
