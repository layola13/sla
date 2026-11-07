# 565 tri_cat

`C(6)=132` 三向收束：卷积递推 `==` 商式 `B(6)/7 ==` Dyck 网格 DP（`132` 对 `132` 对 `132`）。

- 对标：556_cat_conv × 351_catalan × 559_dyck → 三源单实现见各源（316 系三向体例）
- 绕行：三表各自独立（545 同例）；另断言 `dyck(5)=42` 承接 559

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/565_tri_cat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/565_tri_cat/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/565_tri_cat/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/565_tri_cat/main.sla -o /tmp/basic_565 && /tmp/basic_565
# 期望输出：132,132,132
```
