# 905 tri_schro11

大 Schröder 续三向二：迭代 × 商式 × 倍小（`n=11：5293446` 三锁）。

- 对标：896_schro11 → 单格源互补（三实现互锁；795 三向体例收束 Schröder 续三行族）
- 绕行：三实现皆探针三线绿（迭代/商式/倍小秒级会师）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/905_tri_schro11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/905_tri_schro11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/905_tri_schro11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/905_tri_schro11/main.sla -o /tmp/basic_905 && /tmp/basic_905
# 期望输出：5293446,5293446,5293446
```
