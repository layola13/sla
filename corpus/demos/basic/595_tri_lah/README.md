# 595 tri_lah

`L(5,3)=120` 三向收束：商式A `C(n-1,k-1)·n!/k! ==` 商式B `C(n,k)·(n-1)!/(k-1)! ==` 递推（`120` 对 `120` 对 `120`）。

- 对标：586_lah × 589_lah_rec → 两源见各源（商式B为第三源；316 系三向体例）
- 绕行：商式B另断言 `lah_b(4,2)=36`（`6·6` 手算复核）；三源各自独立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/595_tri_lah/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/595_tri_lah/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/595_tri_lah/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/595_tri_lah/main.sla -o /tmp/basic_595 && /tmp/basic_595
# 期望输出：120,120,120
```
