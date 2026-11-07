# 848 duel_narcol19

Narayana 边对峙：商式 vs 三角数 `n(n-1)/2`（`(8,1)=28` 双锁，`(9,1)=36` 双锁）。

- 对标：722_nar_edge → 低位边值互补（边恒等 `N(n,1)=C(n,2)` 纵深三值锁定）
- 绕行：无（纯整数恒等，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/848_duel_narcol19/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/848_duel_narcol19/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/848_duel_narcol19/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/848_duel_narcol19/main.sla -o /tmp/basic_848 && /tmp/basic_848
# 期望输出：28,28,36,36
```
