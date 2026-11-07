# 385 tri pow3sum

3 幂和三向对峙（循环 vs 闭式 vs 逆序；375 的等比版）。

- 对标：375 tri_altsum → `(3^n-1)/2` 闭式锁定（`n=4 → 40` 三向）
- 绕行：无（逆序先重建 `3^n` 再逐项整除回退；整除精确）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/385_tri_pow3sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/385_tri_pow3sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/385_tri_pow3sum/main.sla -o /tmp/basic_385 && /tmp/basic_385
# 期望输出：40,40,40
```
