# 441 euclid steps

欧几里得步数（`(240,46) → 5`；347 的步数版）。

- 对标：347 gcd_chain → 取模轮次计数（`(17,5) → 3`）
- 绕行：无（纯 `int` 循环；计数器同步递增）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/441_euclid_steps/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/441_euclid_steps/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/441_euclid_steps/main.sla -o /tmp/basic_441 && /tmp/basic_441
# 期望输出：5,3,3
```
