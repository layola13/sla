# 443 pell

Pell 数（`P(6)=70`；442 的二倍递推版）。

- 对标：442 lucas → `P(n)=2P(n-1)+P(n-2)`（`P(4)=12`）
- 绕行：`n=0/1` 提前返回（442 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/443_pell/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/443_pell/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/443_pell/main.sla -o /tmp/basic_443 && /tmp/basic_443
# 期望输出：70,12,2
```
