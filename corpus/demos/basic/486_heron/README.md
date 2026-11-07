# 486 heron

Heron 面积（`(13,14,15) → 84`；338 的面积版）。

- 对标：338 pyth_check → 牛顿整数开方（`s(s-a)(s-b)(s-c)` 完全平方）
- 绕行：周长偶数选例（`s` 整数；`isqrt` 牛顿迭代，117 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/486_heron/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/486_heron/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/486_heron/main.sla -o /tmp/basic_486 && /tmp/basic_486
# 期望输出：84,12,12
```
