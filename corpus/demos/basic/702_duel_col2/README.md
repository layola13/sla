# 702 duel_col2

Stirling 二类列对峙：建表 vs 幂公式（`(8,2)：127 对 127`；`(9,2)：255 对 255`）。

- 对标：701_stir2_col2 → 列源互补（幂公式源合流；跨算法双边锁定）
- 绕行：`2^7-1=127` 连乘八步（`>>` 无 token，515 口径）；公式端点 `p-1` 正数差

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/702_duel_col2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/702_duel_col2/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/702_duel_col2/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/702_duel_col2/main.sla -o /tmp/basic_702 && /tmp/basic_702
# 期望输出：127,127,255,255
```
