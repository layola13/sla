# 401 lcm gcd id

lcm·gcd 恒等式（`lcm·gcd = a·b`；348 的恒等式版）。

- 对标：348 lcm_chain → 恒等式锁定（`(12,18) → 216`）
- 绕行：无（先除后乘防溢出；纯 `int` 运算）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/401_lcm_gcd_id/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/401_lcm_gcd_id/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/401_lcm_gcd_id/main.sla -o /tmp/basic_401 && /tmp/basic_401
# 期望输出：216,216,24
```
