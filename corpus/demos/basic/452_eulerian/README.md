# 452 eulerian

Eulerian 数（`A(4,1)=11`；451 的排列版）。

- 对标：451 bell → 上升数递推（`A(4,2)=11` 对称，`A(3,1)=4`）
- 绕行：递归四基（`m==0` 归 1 含 `A(n,0)=1`，`m>=n` 归 0）；小规模安全

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/452_eulerian/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/452_eulerian/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/452_eulerian/main.sla -o /tmp/basic_452 && /tmp/basic_452
# 期望输出：11,11,4
```
