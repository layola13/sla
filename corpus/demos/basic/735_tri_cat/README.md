# 735 tri_cat

Catalan 三向：商式 × 卷积 × 比值递推（`n=7：429 三锁`）。

- 对标：727_cat_div8 → 商源互补（605 三向体例；三算法互锁收束 Catalan 恒等族）
- 绕行：三源异构（商/卷积/比值）同值锁定；`catr(6)=132` 低位陪锁

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/735_tri_cat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/735_tri_cat/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/735_tri_cat/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/735_tri_cat/main.sla -o /tmp/basic_735 && /tmp/basic_735
# 期望输出：429,429,429
```
