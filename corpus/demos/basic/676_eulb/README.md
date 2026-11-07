# 676 eulb

Eulerian-B 型建表 `B(n,m)=(2m+1)·B(n-1,m)+(2n-2m+1)·B(n-1,m-1)`（546 范式）。
`B(3,1)=23`，`B(4,1)=76`，`B(4,2)=230`。

- 对标：666_eulerian → A 型姐妹（符号置换计数，行和 `2^n·n!`；679 做建表/交错和对峙）
- 绕行：初值记忆以行和仲裁纠偏（`B(2,1)=6` 非 `4`，和 `8=2^2·2!`）；`B(2,1)=6` 端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/676_eulb/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/676_eulb/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/676_eulb/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/676_eulb/main.sla -o /tmp/basic_676 && /tmp/basic_676
# 期望输出：23,76,230
```
