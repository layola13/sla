# 916 eul9

Eulerian 续高位三 `E(9,1)=502`，`E(9,2)=14608`，`E(9,3)=88234`（同式再上一行）。

- 对标：798_eul8 → `n=8` 版互补（同式再上一行；918 做建表/交错对峙）
- 绕行：`2^9-9-1=502` 首列恒等手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/916_eul9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/916_eul9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/916_eul9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/916_eul9/main.sla -o /tmp/basic_916 && /tmp/basic_916
# 期望输出：502,14608,88234
```
