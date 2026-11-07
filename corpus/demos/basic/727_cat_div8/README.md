# 727 cat_div8

Catalan 商式 `C(7)=429`，`C(8)=1430`（`C(2n,n)/(n+1)` 精确整除）。

- 对标：351_catalan → 低位商式版互补（同式再上三位；728 做卷积/商式对峙）
- 绕行：`C(16,8)/9=1430` 大值精确整除（596 口径；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/727_cat_div8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/727_cat_div8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/727_cat_div8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/727_cat_div8/main.sla -o /tmp/basic_727 && /tmp/basic_727
# 期望输出：429,1430
```
