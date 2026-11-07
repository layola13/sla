# 555 tri_binom

`C(10,5)=252` 三向收束：Pascal 查表 `==` 乘法式 `==` Vandermonde 自卷积 `Σ_k C(5,k)²`（`252` 对 `252` 对 `252`）。

- 对标：546_pascal × 350_binom × 549_vander → 三源单实现见各源（316 系三向体例）
- 绕行：自卷积另断言 `vander_self(4)=70`（`1+16+36+16+1` 手算复核）；`c*c` 同型相乘无混算坑

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/555_tri_binom/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/555_tri_binom/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/555_tri_binom/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/555_tri_binom/main.sla -o /tmp/basic_555 && /tmp/basic_555
# 期望输出：252,252,252
```
