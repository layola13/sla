# 865 tri_cat9

Catalan 续三向：商式 × 卷积 × 比值递推（`n=9：4862` 三锁）。

- 对标：856_cat9 → 商源互补（三实现互锁；735 三向体例收束 Catalan 续三行族）
- 绕行：五实现六档互验（商/卷积/中央商表/差式/比值，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/865_tri_cat9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/865_tri_cat9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/865_tri_cat9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/865_tri_cat9/main.sla -o /tmp/basic_865 && /tmp/basic_865
# 期望输出：4862,4862,4862
```
