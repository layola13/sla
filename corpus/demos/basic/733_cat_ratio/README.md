# 733 cat_ratio

Catalan 比值递推 `C(n+1)=C(n)·2(2n+1)/(n+2)`（`C(6)=132`，`C(7)=429`，`C(8)=1430`）。

- 对标：351_catalan → 商式版互补（异构比值单锁；735 做三向收束）
- 绕行：逐轮精确整除（`132·26/8=429`；596 口径）；`C(5)=42` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/733_cat_ratio/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/733_cat_ratio/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/733_cat_ratio/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/733_cat_ratio/main.sla -o /tmp/basic_733 && /tmp/basic_733
# 期望输出：132,429,1430
```
