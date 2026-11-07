# 755 tri_motz9

Motzkin 续三向：迭代 × 直和 × 桥序和（`n=9：835 三锁`）。

- 对标：746_motz9 → 单格源互补（635 三向体例；三算法互锁收束 Motzkin 续高位族）
- 绕行：桥序和 `232+603=835` 跨族第三锁（751 体例；`moz(8)=323` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/755_tri_motz9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/755_tri_motz9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/755_tri_motz9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/755_tri_motz9/main.sla -o /tmp/basic_755 && /tmp/basic_755
# 期望输出：835,835,835
```
