# 841 lah_edge9

Lah 边值 `L(9,9)=1`，`L(9,8)=72=9·8`，`L(9,7)=2016`（对角边恒等 `L(n,n-1)=n(n-1)`）。

- 对标：713_lah_edge → 低位边值互补（边恒等 `8·7=56` 连验；845 做三向收束）
- 绕行：`C(8,7)·9!/40320=72` 商式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/841_lah_edge9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/841_lah_edge9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/841_lah_edge9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/841_lah_edge9/main.sla -o /tmp/basic_841 && /tmp/basic_841
# 期望输出：1,72,2016
```
