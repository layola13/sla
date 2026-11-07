# 605 tri_narcat

`C(5)=42` 三向收束：Narayana 行和 `==` Catalan 卷积 `==` 商式 `B(5)/6`（`42` 对 `42` 对 `42`；Catalan 弧收官）。

- 对标：598_narayana6 × 556_cat_conv × 351_catalan → 三源单实现见各源（316 系三向体例）
- 绕行：三源各自独立（545 同例）；另断言 `nar_row(4)=14` 承接 597

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/605_tri_narcat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/605_tri_narcat/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/605_tri_narcat/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/605_tri_narcat/main.sla -o /tmp/basic_605 && /tmp/basic_605
# 期望输出：42,42,42
```
