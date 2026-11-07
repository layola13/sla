# 446 jacobsthal

Jacobsthal 数（`J(7)=43`；442 的加权版）。

- 对标：442 lucas → `J(n)=J(n-1)+2J(n-2)`（`J(5)=11`）
- 绕行：`n=0/1` 提前返回（442 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/446_jacobsthal/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/446_jacobsthal/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/446_jacobsthal/main.sla -o /tmp/basic_446 && /tmp/basic_446
# 期望输出：43,11,1
```
