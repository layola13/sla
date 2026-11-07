# 731 cat_diff

Catalan 反射差式 `C(n)=C(2n,n)-C(2n,n+1)`（`C(4)=14`，`C(5)=42`，`C(6)=132`）。

- 对标：351_catalan → 商式版互补（异构差式单锁；735 做三向收束）
- 绕行：`70-56=14` 正数差（436 口径）；`catdf(3)=5` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/731_cat_diff/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/731_cat_diff/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/731_cat_diff/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/731_cat_diff/main.sla -o /tmp/basic_731 && /tmp/basic_731
# 期望输出：14,42,132
```
