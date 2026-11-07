# 795 tri_schro9

大 Schröder 续三向：迭代 × 商式 × 倍小（`n=9：206098 三锁`）。

- 对标：786_schro9 → 单格源互补（775 三向体例；三算法互锁收束大续族）
- 绕行：倍小 `2·103049=206098` 跨族第三锁（794 桥恒等复用；`sch(8)=41586` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/795_tri_schro9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/795_tri_schro9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/795_tri_schro9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/795_tri_schro9/main.sla -o /tmp/basic_795 && /tmp/basic_795
# 期望输出：206098,206098,206098
```
