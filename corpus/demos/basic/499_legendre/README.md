# 499 legendre

Legendre 符号（`(3|7)=-1` 以 6 表之；405 的符号版）。

- 对标：405 duel_euler → 欧拉准则单边（`(2|7)=1`，`(7|7)=0`）
- 绕行：`-1` 以 `p-1` 表之（`0-1` 亦可，此处取同余类值不断言符号）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/499_legendre/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/499_legendre/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/499_legendre/main.sla -o /tmp/basic_499 && /tmp/basic_499
# 期望输出：1,6,0
```
