# 388 primepow

素数幂判定（`8 → 1`，`12 → 0`；376 的单基版）。

- 对标：376 semiprime → 最小素因子除尽即判（素数本身视为 `p¹`）
- 绕行：单语句 `while` 体后不加 `;`（386 同例；写法约束）
- 绕行：无（试除定位基底后除尽验证）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/388_primepow/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/388_primepow/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/388_primepow/main.sla -o /tmp/basic_388 && /tmp/basic_388
# 期望输出：1,1,0,1
```
