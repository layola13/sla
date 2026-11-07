# 460 duel lazy

懒人厨师双实现对峙（闭式 vs 逐刀累加；288 的切分版）。

- 对标：288 duel_sqsum → `L(n)=L(n-1)+n` 锁定（`5 → 16=16`）
- 绕行：无（首项 1；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/460_duel_lazy/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/460_duel_lazy/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/460_duel_lazy/main.sla -o /tmp/basic_460 && /tmp/basic_460
# 期望输出：16,16
```
