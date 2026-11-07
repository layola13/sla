# 478 farey len

Farey 序列长度（`|F(5)|=11`；373 的互素版）。

- 对标：373 prime_count → `1+Σφ` 公式（`|F(8)|=23`，`phi(1)=1` 复用）
- 绕行：`phi(1)=1` 约定复用（382 口径）；纯 `int` 累加

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/478_farey_len/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/478_farey_len/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/478_farey_len/main.sla -o /tmp/basic_478 && /tmp/basic_478
# 期望输出：11,23,2
```
