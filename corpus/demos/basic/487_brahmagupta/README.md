# 487 brahmagupta

Brahmagupta 面积（`(2,2,3,3) → 6`；486 的四边版）。

- 对标：486 heron → 四因子乘积开方（周长偶数选例）
- 绕行：`isqrt` 复用牛顿迭代（486 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/487_brahmagupta/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/487_brahmagupta/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/487_brahmagupta/main.sla -o /tmp/basic_487 && /tmp/basic_487
# 期望输出：6,16
```
