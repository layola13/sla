# 447 pell comp

Pell 伴随数（`Q(5)=82`；443 的伴随版）。

- 对标：443 pell → 初值 `(2,2)` 同递推（`Q(3)=14`）
- 绕行：`n=0/1` 提前返回（443 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/447_pell_comp/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/447_pell_comp/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/447_pell_comp/main.sla -o /tmp/basic_447 && /tmp/basic_447
# 期望输出：82,14,2
```
