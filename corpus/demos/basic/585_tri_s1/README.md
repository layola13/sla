# 585 tri_s1

`row5=120` 三向收束：表行和 `==` 阶乘闭式 `==` 上升直乘 `rise(5,1)`（`120` 对 `120` 对 `120`）。

- 对标：580_rowval × 125_fact × 577_rising → 三源单实现见各源（316 系三向体例）
- 绕行：三源各自独立建表/循环（545 同例）；另断言 `s1_row(4)=24` 承接 580

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/585_tri_s1/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/585_tri_s1/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/585_tri_s1/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/585_tri_s1/main.sla -o /tmp/basic_585 && /tmp/basic_585
# 期望输出：120,120,120
```
