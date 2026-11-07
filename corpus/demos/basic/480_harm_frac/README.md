# 480 harm frac

调和和既约分数（`H(4)=25/12`；362 的既约版）。

- 对标：362 harm_bound → 通分约分循环（`H(3)=11/6`）
- 绕行：步步约分防溢出；分子分母双函数（477 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/480_harm_frac/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/480_harm_frac/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/480_harm_frac/main.sla -o /tmp/basic_480 && /tmp/basic_480
# 期望输出：2512,1106,1
```
