# 514 parity

奇偶校验位（`7 → 1`；508 的奇偶版）。

- 对标：508 bitcount → `count%2` 判定（`15 → 0`）
- 绕行：`1/0` 代替布尔（346 同例）；复用 `bitcount`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/514_parity/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/514_parity/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/514_parity/main.sla -o /tmp/basic_514 && /tmp/basic_514
# 期望输出：1,0,1
```
