# 509 bitrev

字节位反转（`18 → 72`；409 的位版）。

- 对标：409 rev_num → 余数左移拼装（`1 → 128`，`255 → 255` 对称）
- 绕行：定 8 轮（`%2` 取位 `/2` 右移；避 `>>` 未知算子）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/509_bitrev/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/509_bitrev/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/509_bitrev/main.sla -o /tmp/basic_509 && /tmp/basic_509
# 期望输出：72,128,255
```
