# 450 perrin

Perrin 数（`R(10)=17`；449 的异初值版）。

- 对标：449 padovan → 同递推异初值 `(3,0,2)`（`R(5)=5`）
- 绕行：`n=0/1/2` 三提前返回（449 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/450_perrin/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/450_perrin/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/450_perrin/main.sla -o /tmp/basic_450 && /tmp/basic_450
# 期望输出：17,5,0
```
