# 826 stir29

Stirling 二类第 10 行左部 `S(10,2)=511`，`S(10,3)=9330`，`S(10,4)=34105`（同式再上三行）。

- 对标：696_stir27 → `n=7` 版互补（同式再上三行；829 做建表/递归对峙）
- 绕行：`127+3·966=3025` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/826_stir29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/826_stir29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/826_stir29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/826_stir29/main.sla -o /tmp/basic_826 && /tmp/basic_826
# 期望输出：511,9330,34105
```
