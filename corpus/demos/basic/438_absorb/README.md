# 438 absorb

gcd/lcm 吸收律（`gcd(4,lcm)=4`；401 的格版）。

- 对标：401 lcm_gcd_id → 吸收恒等式双向（`(12,18)` 组复核）
- 绕行：无（调用嵌套；纯 `int` 运算）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/438_absorb/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/438_absorb/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/438_absorb/main.sla -o /tmp/basic_438 && /tmp/basic_438
# 期望输出：4,4,12
```
