# 393 binom odd

二项式奇偶 Lucas（`(k&n)==k` 即奇；350 的奇偶版）。

- 对标：350 binom → 位运算判定（`C(5,2)=10` 偶，`C(3,1)=3` 奇）
- 绕行：`&` 加括号（145 优先级坑口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/393_binom_odd/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/393_binom_odd/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/393_binom_odd/main.sla -o /tmp/basic_393 && /tmp/basic_393
# 期望输出：0,1,0,1
```
