# 434 duel quad

四元组双实现对峙（直接判定 vs 重叠三元组；423 的素串版）。

- 对标：423 duel_sphenic → `(p,p+2,p+6)+(p+2,p+6,p+8)` 结构锁定（`5/11 → 1=1`）
- 绕行：无（四元组恒为两重叠三元组之并；纯 `int` 判定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/434_duel_quad/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/434_duel_quad/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/434_duel_quad/main.sla -o /tmp/basic_434 && /tmp/basic_434
# 期望输出：1,1,1
```
