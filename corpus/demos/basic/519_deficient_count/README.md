# 519 deficient count

亏数计数（`<=20 → 16`；378 的亏版）。

- 对标：378 abundant → 亏盈分类计数（`20-3-1=16` 复核）
- 绕行：`n < 1` 守卫（410 同例）；复用 `sigma`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/519_deficient_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/519_deficient_count/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/519_deficient_count/main.sla -o /tmp/basic_519 && /tmp/basic_519
# 期望输出：16,1,0
```
