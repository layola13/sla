# 834 stir29_right

Stirling 二类第 10 行右部 `S(10,7)=5880`，`S(10,8)=750`，`S(10,9)=45`（826 左部版互补，全行覆盖）。

- 对标：826_stir29 → 左部版互补（左右合流全行 `1,511,9330,34105,42525,22827,5880,750,45,1`）
- 绕行：`266+7·28=462` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/834_stir29_right/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/834_stir29_right/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/834_stir29_right/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/834_stir29_right/main.sla -o /tmp/basic_834 && /tmp/basic_834
# 期望输出：5880,750,45
```
