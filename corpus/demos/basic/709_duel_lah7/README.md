# 709 duel_lah7

Lah 双实现对峙：建表 vs 商式（`(7,3)：12600 对 12600`；`(7,4)：4200 对 4200`）。
商式 `L(n,k)=C(n-1,k-1)·n!/k!`（先乘后除精确整除）。

- 对标：706_lah7 → 建表源（商式版互补；286 系对峙体例，双边同值锁定）
- 绕行：`15·5040/6=12600` 先乘后除精确（596 口径）；`k=0` 提前返零守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/709_duel_lah7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/709_duel_lah7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/709_duel_lah7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/709_duel_lah7/main.sla -o /tmp/basic_709 && /tmp/basic_709
# 期望输出：12600,12600,4200,4200
```
