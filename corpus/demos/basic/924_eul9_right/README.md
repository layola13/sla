# 924 eul9_right

Eulerian 第 9 行右部 `E(9,5)=88234`，`E(9,6)=14608`，`E(9,7)=502`（916 左部版互补，全行覆盖）。

- 对标：916_eul9 → 左部版互补（左右合流全行 `1,502,14608,88234,156190,88234,14608,502,1`）
- 绕行：对称恒等 `E(9,7)=E(9,1)` 双算一致（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/924_eul9_right/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/924_eul9_right/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/924_eul9_right/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/924_eul9_right/main.sla -o /tmp/basic_924 && /tmp/basic_924
# 期望输出：88234,14608,502
```
