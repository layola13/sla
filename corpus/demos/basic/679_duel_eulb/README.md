# 679 duel_eulb

Eulerian-B 双实现对峙：建表 vs 交错和（`(4,2)：230 对 230`；`(5,2)：1682 对 1682`）。
交错和 `B(n,m)=sum (-1)^k·C(n+1,k)·(2(m-k)+1)^n`（`(5,2)：3125-1458+15=1682`）。

- 对标：676_eulb → 建表源（交错和版互补；286 系对峙体例，双边同值锁定）
- 绕行：`%2` 判奇偶（515 口径）；奇底数幂天然正（`1^5=1` 尾项守卫）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/679_duel_eulb/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/679_duel_eulb/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/679_duel_eulb/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/679_duel_eulb/main.sla -o /tmp/basic_679 && /tmp/basic_679
# 期望输出：230,230,1682,1682
```
