# 861 cat_diff9

Catalan 反射差式高位 `C(9)=4862`，`C(10)=16796`（`C(2n,n)-C(2n,n+1)`，正数差）。

- 对标：731_cat_diff → 低位差式版互补（同式再上高位；865 做三向收束）
- 绕行：`184756-167960=16796` 正数差手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/861_cat_diff9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/861_cat_diff9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/861_cat_diff9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/861_cat_diff9/main.sla -o /tmp/basic_861 && /tmp/basic_861
# 期望输出：4862,16796,1430
```
