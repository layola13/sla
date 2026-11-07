# 412 duel rev

反转双实现对峙（循环 vs 尾递归；411 的反转版）。

- 对标：411 duel_dsum → 累积器尾递归（`12345 → 54321` 双边）
- 绕行：递归累积器显式传参（无默认参数，#8 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/412_duel_rev/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/412_duel_rev/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/412_duel_rev/main.sla -o /tmp/basic_412 && /tmp/basic_412
# 期望输出：54321,54321
```
