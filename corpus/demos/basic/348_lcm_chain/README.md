# 348 lcm chain

多值 lcm 链（`lcm(a,b) = a/gcd*b`；21 的链式版，91 互补）。

- 对标：21 gcd/lcm → 三元链
- 绕行：无（先除后乘防溢出；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/348_lcm_chain/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/348_lcm_chain/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/348_lcm_chain/main.sla -o /tmp/basic_348 && /tmp/basic_348
# 期望输出：12,24,35
```
