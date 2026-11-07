# 779 duel_bell9

Bell 双实现对峙：二项递推 vs Aitken 三角（`n=8：4140 对 4140`；`n=9：21147 对 21147`）。

- 对标：776_bell9 → 递推源（三角版互补；286 系对峙体例，双边同值锁定）
- 绕行：三角对角线传播（`T(n,0)=T(n-1,n-1)`；左列即 Bell，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/779_duel_bell9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/779_duel_bell9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/779_duel_bell9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/779_duel_bell9/main.sla -o /tmp/basic_779 && /tmp/basic_779
# 期望输出：4140,4140,21147,21147
```
