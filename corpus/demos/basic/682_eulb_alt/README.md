# 682 eulb_alt

Eulerian-B 交错和 `B(n,m)=sum (-1)^k·C(n+1,k)·(2(m-k)+1)^n`（`B(3,1)=23`，`B(4,1)=76`，`B(5,2)=1682`）。

- 对标：676_eulb → 建表版互补（异构交错单锁；685 做三向收束）
- 绕行：`(3,1)` 两项 `27-4=23` 手算复核（探针三线绿）；`eba(2,1)=6` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/682_eulb_alt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/682_eulb_alt/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/682_eulb_alt/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/682_eulb_alt/main.sla -o /tmp/basic_682 && /tmp/basic_682
# 期望输出：23,76,1682
```
