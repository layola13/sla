# 672 eul_alt

Eulerian 交错和 `E(n,m)=sum (-1)^k·C(n+1,k)·(m+1-k)^n`（`E(4,1)=11`，`E(5,2)=66`，`E(6,3)=302`）。

- 对标：666_eulerian → 建表版互补（异构交错单锁；675 做三向收束）
- 绕行：`(6,3)` 五项 `4096-5103+1344-35+0=302` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/672_eul_alt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/672_eul_alt/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/672_eul_alt/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/672_eul_alt/main.sla -o /tmp/basic_672 && /tmp/basic_672
# 期望输出：11,66,302
```
