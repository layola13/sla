# 513 nibble swap

半字节交换（`18 → 33`；509 的半字节版）。

- 对标：509 bitrev → 高低四位互换（`171 → 186`，`255 → 255`）
- 绕行：`%16` 与 `/16` 拆装（避位移算子；纯 `int` 运算）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/513_nibble_swap/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/513_nibble_swap/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/513_nibble_swap/main.sla -o /tmp/basic_513 && /tmp/basic_513
# 期望输出：33,186,255
```
