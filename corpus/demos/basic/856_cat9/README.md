# 856 cat9

Catalan 商式高位 `C(9)=4862`，`C(10)=16796`（`C(2n,n)/(n+1)` 先乘后除精确整除）。

- 对标：727_cat_div8 → `n=7/8` 版互补（同式再上两位；858 做卷积/商式对峙）
- 绕行：`184756/11=16796` 分子分母手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/856_cat9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/856_cat9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/856_cat9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/856_cat9/main.sla -o /tmp/basic_856 && /tmp/basic_856
# 期望输出：4862,16796
```
